"""ICAD 2008 的有限原生修改：固定 Undo 指令與程式產生的半徑／ACI 修改。

舊 COM 屬性寫入未能納入原生 Undo，因此不能只補 Begin／End 後照用 COM。
本模組只接受已驗證的圖元與數字，不接受模型 LISP、指令或路徑；原生修改
須取得帶 nonce 的完成回條。失敗不自動 Undo，也不儲存或關閉目前圖面。
"""
import ctypes
import json
from pathlib import Path
import re
import tempfile
import time
import uuid


def prepare(edits, rows):
    """整批預先檢查範圍，避免執行到一半才發現新版功能不支援。"""
    plan = []
    for edit in edits:
        handle, kind, value = edit['handle'], edit['kind'], edit['value']
        if not isinstance(handle, str) or not re.fullmatch(r'[0-9A-Fa-f]{1,64}', handle):
            raise ValueError('原生修改的 Handle 不合法')
        row = rows[handle]
        if kind == 'radius' and row['kind'] == 'circle':
            code, old = 40, row['properties']['Radius']
        elif kind == 'color':
            code, old = 62, row['properties']['Color']
        else:
            # 讓正式 worker 回報 unsupported_capability，在任何寫入之前停止。
            raise AttributeError('ICAD 2008 原生 Undo 備援目前僅支援圓半徑及 ACI 顏色修改')
        plan.append(dict(handle=handle.upper(), code=code, old=old, value=value))
    return plan


def lisp_string(value):
    """僅插入程式讀回的路徑／文件名稱；禁止控制字元與未跳脫引號。"""
    if not isinstance(value, str) or any(ord(c) < 32 for c in value):
        raise ValueError('原生文件或路徑字串不合法')
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'


def source(plan, name, prefix, units, output, nonce):
    """固定模板；每個插入值只能是已驗證的 Handle、字串或 JSON 有限數字。"""
    def number(value):
        if type(value) not in (int, float):
            raise ValueError('原生修改只接受數字')
        return json.dumps(value, allow_nan=False)

    items = ' '.join('(' + ' '.join((lisp_string(p['handle']), str(p['code']),
        number(p['old']), number(p['value']))) + ')' for p in plan)
    template = '''
(defun lm-edit-receipt (status / f)
  (setq f (open @@OUTPUT@@ "w"))
  (if f (progn (prin1 (list @@NONCE@@ status lm-edit-count) f) (close f))))
(defun lm-edit-error (message)
  (lm-edit-receipt "ERROR") (setq *error* lm-edit-old-error) (princ))
(defun lm-edit-run (/ e d p actual item replacement updated lm-edit-plan lm-edit-valid
                       lm-edit-ok lm-edit-old-error lm-edit-count)
(setq lm-edit-old-error *error* lm-edit-count 0)
(setq *error* lm-edit-error)
(setq lm-edit-plan '@@PLAN@@)
(setq lm-edit-valid (and (= (getvar "DWGNAME") @@NAME@@) (= (getvar "DWGPREFIX") @@PREFIX@@)
                         (= (getvar "INSUNITS") @@UNITS@@) (/= 0 (logand 8 (getvar "UNDOCTL")))))
(foreach item lm-edit-plan
  (setq e (handent (car item)) d (if e (entget e) nil))
  (setq p (if d (assoc (cadr item) d) nil))
  (setq actual (if p (cdr p) (if (= (cadr item) 62) 256 nil)))
  (if (not (and d (or (/= (cadr item) 62) (and (not (assoc 420 d)) (not (assoc 430 d))))
                (or (not (assoc 67 d)) (= 0 (cdr (assoc 67 d))))
                (or (/= (cadr item) 40) (= "CIRCLE" (cdr (assoc 0 d))))
                (= 0 (logand 4 (cdr (assoc 70 (tblsearch "LAYER" (cdr (assoc 8 d)))))))
                (equal actual (caddr item) 0.0000001)))
      (setq lm-edit-valid nil)))
(if lm-edit-valid
  (progn
    (setq lm-edit-ok T)
    (foreach item lm-edit-plan
      (if lm-edit-ok
        (progn
          (setq e (handent (car item)) d (entget e) p (assoc (cadr item) d))
          (setq replacement (cons (cadr item) (cadddr item)))
          (setq updated (if p (subst replacement p d) (append d (list replacement))))
          (if (entmod updated)
            (progn (setq lm-edit-count (1+ lm-edit-count)) (entupd e))
            (setq lm-edit-ok nil)))))
    (lm-edit-receipt (if lm-edit-ok "OK" "ERROR")))
  (lm-edit-receipt "PRECONDITION"))
(setq *error* lm-edit-old-error)
(princ))
(lm-edit-run)
(princ)
'''
    values = dict(OUTPUT=lisp_string(output.as_posix()), NONCE=lisp_string(nonce),
                  PLAN='(' + items + ')', NAME=lisp_string(name), PREFIX=lisp_string(prefix), UNITS=number(units))
    # 一次替換，文件名稱恰含占位文字時也不會被二次展開。
    return re.sub(r'@@(OUTPUT|NONCE|PLAN|NAME|PREFIX|UNITS)@@', lambda m: values[m.group(1)], template)


def run_command(app, doc, arguments, grouped, guard, legacy=True):
    """只允許固定 Begin／End；等待群組旗標，而不是把 COM 返回當作完成。"""
    if arguments not in ('_Begin', '_End'):
        raise ValueError('只允許固定 Undo 群組指令')
    guard()
    command = '_.UNDO\n' + arguments + '\n'
    if legacy:
        app.RunCommand(command)
    else:
        doc.SendCommand(command)
    import pythoncom
    deadline = time.monotonic() + 15
    while True:
        guard()
        ctl = int(doc.GetVariable('UNDOCTL'))
        if int(doc.GetVariable('CMDACTIVE')) == 0 and bool(ctl & 8) == grouped:
            return
        if time.monotonic() >= deadline:
            raise TimeoutError('原生 Undo 群組沒有確認完成；不重送')
        pythoncom.PumpWaitingMessages()
        time.sleep(.05)


def begin_group(app, doc, report, guard, legacy=False):
    """原生群組跨過內部批次讀取，避免2022的COM標記被SendCommand分段。"""
    guard()
    ctl = int(doc.GetVariable('UNDOCTL'))
    if not ctl & 1 or ctl & (2 | 8) or int(doc.GetVariable('CMDACTIVE')) != 0:
        raise ValueError('CAD 尚未就緒或已有 Undo 群組，未開始修改；請完成目前操作')
    method = getattr(app, 'RunCommand', None) if legacy else getattr(doc, 'SendCommand', None)
    if not callable(method):
        raise AttributeError('CAD 缺少已驗證的原生 Undo 群組介面，未修改')
    report.update(undo_group_started=False, undo_group_closed=False,
                  undo_method='icad_runcommand_native' if legacy else 'native_command_group_with_com_edits')

    def finish_group():
        guard()
        if int(doc.GetVariable('UNDOCTL')) & 8:
            run_command(app, doc, '_End', False, guard, legacy)
        report['undo_group_closed'] = not bool(int(doc.GetVariable('UNDOCTL')) & 8)

    try:
        run_command(app, doc, '_Begin', True, guard, legacy)
        report['undo_group_started'] = True
    except BaseException:
        finish_group()
        raise
    return finish_group


def apply(app, doc, edits, rows, report, guard):
    """只修改已確認的目前文件，完成後由既有引擎完整核對全部圖元。"""
    plan = prepare(edits, rows)
    guard()
    if not callable(getattr(app, 'RunCommand', None)) or not callable(getattr(app, 'LoadLISP', None)):
        raise AttributeError('ICAD 缺少已驗證的原生 Undo／LISP 介面，未修改')
    folder = Path(tempfile.mkdtemp(prefix='LM_AI_CAD_EDIT_'))
    output, script = folder / 'complete.txt', folder / 'edit.lsp'
    nonce = uuid.uuid4().hex
    text = source(plan, str(doc.GetVariable('DWGNAME')), str(doc.GetVariable('DWGPREFIX')),
                  doc.GetVariable('INSUNITS'), output, nonce)
    encoding = 'cp' + str(ctypes.windll.kernel32.GetACP())
    script.write_text(text, encoding=encoding)
    finish_native_group = begin_group(app, doc, report, guard, legacy=True)

    def finish_group():
        """由呼叫端在 Regen／完整核對之後關閉，避免讀回步驟占用下一次 Undo。"""
        finish_native_group()
        if report.get('completed_operations') == len(plan) and report.get('undo_group_closed'):
            for path in (script, output):
                path.unlink(missing_ok=True)
            folder.rmdir()

    try:
        guard()
        report.update(step='native_edit', drawing_started=True)
        app.LoadLISP(str(script))
        import pythoncom
        deadline = time.monotonic() + 60
        receipt = None
        while receipt is None:
            guard()
            if output.exists():
                # 不解析任意 LISP，只接受固定且完整的狀態回條；不使用 eval。
                match = re.fullmatch(r'\("' + nonce + r'" "(OK|ERROR|PRECONDITION)" ([0-9]+)\)\s*',
                                     output.read_text(encoding='ascii'))
                if match:
                    receipt = match.group(1), int(match.group(2))
                    break
            if time.monotonic() >= deadline:
                raise TimeoutError('原生修改沒有完整回條；結果未知，不自動重送')
            pythoncom.PumpWaitingMessages()
            time.sleep(.05)
        status, count = receipt
        if not 0 <= count <= len(plan):
            raise ValueError('原生修改回條計數不符')
        report['completed_operations'] = count
        if status == 'PRECONDITION' and count == 0:
            report['drawing_started'] = False
        if status != 'OK' or count != len(plan):
            raise ValueError('原生修改未完成：' + status + '；不自動復原或重送')
    except BaseException:
        # 只結束本次已開始且仍屬於相同文件的群組，不自動 Undo；未知結果保留診斷。
        finish_group()
        raise
    return finish_group
