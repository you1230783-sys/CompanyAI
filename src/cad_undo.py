"""ICAD2008有限原生修改：半徑／ACI、文字及平面直徑標註。

舊 COM 屬性寫入未能納入原生 Undo，因此不能只補 Begin／End 後照用 COM。
本模組只接受已驗證的圖元、數字及文字，不接受模型 LISP、指令或路徑；原生修改
須取得帶 nonce 的完成回條。失敗不自動 Undo，也不儲存或關閉目前圖面。
"""
import ctypes
import json
import math
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
        elif kind == 'text' and row['kind'] in ('text', 'mtext'):
            old = row.get('native_text')
            if not isinstance(old, str):
                raise AttributeError('缺少原生文字快照，請刷新目前圖面後再修改')
            # 所有編碼與長度檢查在開始Undo之前完成，不接受不完整／有損字串。
            encoding = 'cp' + str(ctypes.windll.kernel32.GetACP())
            from worker import cad_text
            text = cad_text(value)
            old.encode(encoding, errors='strict')
            text.encode(encoding, errors='strict')
            chunks, chunk = [], ''
            for char in text:
                if len((chunk + char).encode(encoding)) > 240:
                    chunks.append([3, chunk])
                    chunk = ''
                chunk += char
            if row['kind'] == 'text' and chunks:
                raise ValueError('單行文字超過原生字串長度，請保留原文並另行處理')
            chunks.append([1, chunk])
            plan.append(dict(handle=handle.upper(), code=1, old=old, value=chunks,
                             entity_type='MTEXT' if row['kind'] == 'mtext' else 'TEXT'))
            continue
        elif kind == 'diameter' and row['kind'] == 'dimension':
            p = row['properties']
            points = diameter_points(p, value)
            plan.append(dict(handle=handle.upper(), code=10,
                old=[p['DiameterStart'], p['DiameterEnd']], value=points, entity_type='DIMENSION'))
            continue
        else:
            # 讓正式 worker 回報 unsupported_capability，在任何寫入之前停止。
            raise AttributeError('ICAD2008原生修改支援圓半徑、ACI、文字及平面直徑標註；不支援刪除／新增')
        plan.append(dict(handle=handle.upper(), code=code, old=old, value=value))
    return plan


def diameter_points(properties, diameter):
    """保持標註中心與方向，真正調整兩個量測端點，不寫假尺寸文字。

    DXF直徑標註的10／15為相對兩點；來源：Autodesk DXF radial/diameter reference。
    """
    start, end = properties['DiameterStart'], properties['DiameterEnd']
    distance = math.dist(start, end)
    if not math.isfinite(distance) or distance <= 0 or not 0 < diameter <= 1e12:
        raise ValueError('直徑標註端點或目標直徑不合法')
    center = [(a+b)/2 for a, b in zip(start, end)]
    return [[c+(v-c)*diameter/distance for c, v in zip(center, point)] for point in (start, end)]


def update_expected(row, edit):
    """只更新已指定欄位；其餘位置、樣式及原生圖元保持既有核對。"""
    p, kind, value = row['properties'], edit['kind'], edit['value']
    if kind == 'diameter':
        p['DiameterStart'], p['DiameterEnd'] = diameter_points(p, value)
        p['MeasuredDiameter'] = value
        if 'Measurement' in p:
            p['Measurement'] = value
        # TextString是CAD依新幾何重算的顯示結果，不能要求仍等於舊的Ø60。
        p.pop('TextString', None)
    elif kind == 'text':
        from worker import cad_text
        p['TextString'] = cad_text(value)
    else:
        p['Radius' if kind == 'radius' else 'Color'] = value


def lisp_string(value):
    """僅插入程式讀回的路徑／文件名稱；禁止控制字元與未跳脫引號。"""
    if not isinstance(value, str) or any(ord(c) < 32 for c in value):
        raise ValueError('原生文件或路徑字串不合法')
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"') + '"'


def source(plan, name, prefix, units, output, nonce):
    """固定模板；插入值限已核對Handle、文字、端點與有限數字，字串一律跳脫。"""
    def number(value):
        if type(value) not in (int, float):
            raise ValueError('原生修改只接受數字')
        return json.dumps(value, allow_nan=False)

    def literal(value):
        if isinstance(value, str):
            return lisp_string(value)
        if isinstance(value, list):
            return '(' + ' '.join(literal(v) for v in value) + ')'
        return number(value)

    items = ' '.join('(' + ' '.join((lisp_string(p['handle']), str(p['code']),
        literal(p['old']), literal(p['value']), lisp_string(p.get('entity_type', '')))) + ')' for p in plan)
    template = '''
(defun lm-edit-receipt (status / f)
  (setq f (open @@OUTPUT@@ "w"))
  (if f (progn (prin1 (list @@NONCE@@ status lm-edit-count lm-edit-replacements) f) (close f))))
(defun lm-edit-error (message)
  (lm-edit-receipt "ERROR") (setq *error* lm-edit-old-error) (princ))
(defun lm-edit-text (d / s pair)
  (setq s "")
  (foreach pair d (if (= (car pair) 3) (setq s (strcat s (cdr pair)))))
  (strcat s (if (assoc 1 d) (cdr (assoc 1 d)) "")))
(defun lm-edit-text-data (d chunks / result inserted pair chunk)
  (setq result nil inserted nil)
  (foreach pair d
    (if (member (car pair) '(1 3))
      (if (not inserted)
        (progn (foreach chunk chunks (setq result (cons (cons (car chunk) (cadr chunk)) result)))
               (setq inserted T)))
      (setq result (cons pair result))))
  (reverse result))
(defun lm-edit-run (/ e d p actual item replacement updated lm-edit-plan lm-edit-valid
                       lm-edit-ok lm-edit-old-error lm-edit-count lm-edit-replacements created pairs pair)
(setq lm-edit-old-error *error* lm-edit-count 0 lm-edit-replacements "")
(setq *error* lm-edit-error)
(setq lm-edit-plan '@@PLAN@@)
(setq lm-edit-valid (and (= (getvar "DWGNAME") @@NAME@@) (= (getvar "DWGPREFIX") @@PREFIX@@)
                         (= (getvar "INSUNITS") @@UNITS@@) (/= 0 (logand 8 (getvar "UNDOCTL")))))
(foreach item lm-edit-plan
  (setq e (handent (car item)) d (if e (entget e) nil))
  (setq p (if d (assoc (cadr item) d) nil))
  (setq actual (if p (cdr p) (if (= (cadr item) 62) 256 nil)))
  (if (= (cadr item) 1) (setq actual (lm-edit-text d)))
  (if (= (cadr item) 10) (setq actual (list (cdr (assoc 10 d)) (cdr (assoc 15 d)))))
  (if (not (and d (or (/= (cadr item) 62) (and (not (assoc 420 d)) (not (assoc 430 d))))
                (or (not (assoc 67 d)) (= 0 (cdr (assoc 67 d))))
                (or (/= (cadr item) 40) (= "CIRCLE" (cdr (assoc 0 d))))
                (or (= (nth 4 item) "") (= (nth 4 item) (cdr (assoc 0 d))))
                (or (/= (cadr item) 10) (and (= 3 (logand 7 (cdr (assoc 70 d))))
                    (not (assoc 102 d)) (not (assoc 360 d))
                    (or (not (assoc 1 d)) (member (cdr (assoc 1 d)) '("" "<>")))))
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
          (if (= (cadr item) 1) (setq updated (lm-edit-text-data d (cadddr item))))
          (if (= (cadr item) 10)
            (progn
              (setq updated (subst (cons 15 (cadr (cadddr item))) (assoc 15 d)
                            (subst (cons 10 (car (cadddr item))) (assoc 10 d) d)))
              ; 2008原位改端點的唯讀42快取不隨Undo還原。安全替換本尺寸，讓Undo還原原物件。
              ; 保留位置／方向／樣式；移除舊身分及計算快取，不接受模型原生資料。
              (setq pairs nil)
              (foreach pair updated
                (if (not (member (car pair) '(-1 5 330 2 42))) (setq pairs (cons pair pairs))))
              (if (entmake (reverse pairs))
                (progn
                  (setq created (entlast))
                  (if (entdel e)
                    (progn (setq lm-edit-count (1+ lm-edit-count))
                      (setq lm-edit-replacements (strcat lm-edit-replacements (car item) ":"
                        (cdr (assoc 5 (entget created))) ";")))
                    (setq lm-edit-ok nil)))
                (setq lm-edit-ok nil)))
            (if (entmod updated)
              (progn (setq lm-edit-count (1+ lm-edit-count)) (entupd e))
              (setq lm-edit-ok nil))))))
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
                match = re.fullmatch(r'\("' + nonce + r'" "(OK|ERROR|PRECONDITION)" ([0-9]+) "([0-9A-Fa-f:;]*)"\)\s*',
                                     output.read_text(encoding='ascii'))
                if match:
                    receipt = match.group(1), int(match.group(2)), match.group(3)
                    break
            if time.monotonic() >= deadline:
                raise TimeoutError('原生修改沒有完整回條；結果未知，不自動重送')
            pythoncom.PumpWaitingMessages()
            time.sleep(.05)
        status, count, replacements = receipt
        if not 0 <= count <= len(plan):
            raise ValueError('原生修改回條計數不符')
        report['completed_operations'] = count
        mapping = {}
        for item in replacements.split(';'):
            if not item:
                continue
            old, new = item.split(':')
            if old not in {p['handle'] for p in plan if p['code'] == 10} or old in mapping or not new:
                raise ValueError('直徑標註替換回條不符')
            mapping[old] = new.upper()
        report['replacement_handles'] = mapping
        if status == 'OK' and len(mapping) != sum(p['code'] == 10 for p in plan):
            raise ValueError('直徑標註替換回條數量不符')
        if status == 'PRECONDITION' and count == 0:
            report['drawing_started'] = False
        if status != 'OK' or count != len(plan):
            raise ValueError('原生修改未完成：' + status + '；不自動復原或重送')
    except BaseException:
        # 只結束本次已開始且仍屬於相同文件的群組，不自動 Undo；未知結果保留診斷。
        finish_group()
        raise
    return finish_group
