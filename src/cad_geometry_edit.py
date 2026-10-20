"""既有二維幾何的固定修改計畫。

修剪／延伸僅處理兩條非平行直線；圓角以兩條直線及各自保留側計算。
先完成整批幾何及副目標檢查，再由固定 LISP 寫入同一 Undo 群組。
"""
import copy
import math
from pathlib import Path
import re
import tempfile
import time
import uuid
from cad_geometry import EDIT_KINDS, vector, dot, cross, length, symbol


def ensure(condition, message):
    if not condition:
        raise ValueError(message)


def fields(value, names):
    ensure(isinstance(value, dict) and set(value) == set(names.split()), '幾何修改的 value 欄位不符')


def point(value):
    from worker import point_value
    return point_value(value)


def scalar(value):
    from worker import number
    return number(value)


def geometry(row):
    """只接受可完整核對的 WCS XY 基本圖元；未知／3D／帶弧段折線均停止。"""
    p = row['properties']
    ensure(row['kind'] in ('line', 'circle', 'arc', 'polyline'), '此修改只支援平面線／圓／弧／直線輕量多段線')
    ensure(p.get('Normal', [0, 0, 1]) == [0, 0, 1], '傾斜法向量不支援此幾何修改')
    positions = [p[k] for k in ('StartPoint', 'EndPoint', 'Center') if k in p]
    if row['kind'] == 'polyline':
        g = row.get('geometry', {})
        ensure(g.get('straight_segments') is True and g.get('coordinates') == 'WCS'
               and row['type'].lower() in ('acdbpolyline', 'lightweightpolyline', 'lwpolyline'), '只支援已核對頂點的直線輕量多段線')
        positions.extend(g['vertices'])
    ensure(positions and all(len(p) == 3 and abs(p[2]) < 1e-9 for p in positions), '幾何修改目前限定 WCS Z=0')
    return copy.deepcopy(row)


def map_geometry(row, transform, mirrored=False):
    result = copy.deepcopy(row)
    p = result['properties']
    for key in ('StartPoint', 'EndPoint', 'Center'):
        if key in p:
            p[key] = transform(p[key])
    if row['kind'] == 'arc':
        if mirrored:
            p['StartPoint'], p['EndPoint'] = p['EndPoint'], p['StartPoint']
        for key, end in [('StartAngle', 'StartPoint'), ('EndAngle', 'EndPoint')]:
            p[key] = math.atan2(p[end][1]-p['Center'][1], p[end][0]-p['Center'][0]) % math.tau
    if 'geometry' in result:
        result['geometry']['vertices'] = [transform(p) for p in result['geometry']['vertices']]
    return result


def line_intersection(first, second):
    a, b = first['properties']['StartPoint'], first['properties']['EndPoint']
    c, d = second['properties']['StartPoint'], second['properties']['EndPoint']
    u, v = vector(a, b), vector(c, d)
    determinant = cross(u, v)
    ensure(abs(determinant) > 1e-9 * max(length(u)*length(v), 1), '平行／退化直線沒有唯一交點')
    t, s = cross(vector(a, c), v)/determinant, cross(vector(a, c), u)/determinant
    return [a[0]+t*u[0], a[1]+t*u[1], 0.0], t, s


def closest_endpoint(row, pick):
    p = row['properties']
    distances = [math.dist(pick, p[key]) for key in ('StartPoint', 'EndPoint')]
    ensure(abs(distances[0]-distances[1]) > 1e-7, '選取點無法確定要處理哪一端，請靠近目標端點指定pick')
    return 'StartPoint' if distances[0] < distances[1] else 'EndPoint'


def prepare(edits, rows, layers):
    """回傳明確的原圖修改及新增副本；不呼叫 CAD、不從生成結果反推預期值。"""
    plans, used = [], set()
    for index, edit in enumerate(edits):
        ensure(set(edit) == {'handle', 'kind', 'value'}, '修改需handle/kind/value')
        handle, kind, value = edit['handle'], edit['kind'], edit['value']
        ensure(kind in EDIT_KINDS, '幾何修改不可與改字／半徑等其他批次混合；請分批明確操作')
        ensure(handle in rows and handle not in used, 'Handle缺少或重複使用')
        row = geometry(rows[handle])
        ensure(row.get('editable') is True, '此圖元未開放修改')
        ensure(kind in row.get('allowed_edits', []), '快照沒有開放此幾何修改，請依allowed_edits操作')
        ensure(row['properties']['Layer'].casefold() in layers and not layers[row['properties']['Layer'].casefold()]['locked'], '來源圖層不存在或已鎖定')
        used.add(handle)

        def add(updated, mode='modify', source=handle):
            plans.append(dict(source=source, old=rows[source] if source else None, updated=updated, mode=mode, operation=index))

        if kind == 'copy':
            fields(value, 'offset')
            delta = point(value['offset'])
            ensure(any(delta), '複製位移不可為零')
            add(map_geometry(row, lambda p: [p[i]+delta[i] for i in range(3)]), 'copy')
        elif kind == 'rotate':
            fields(value, 'base angle')
            base, radians = point(value['base']), math.radians(scalar(value['angle']))
            ensure(abs(radians) > 1e-12, '旋轉角度不可為零')
            c, s = math.cos(radians), math.sin(radians)
            add(map_geometry(row, lambda p: [base[0]+c*(p[0]-base[0])-s*(p[1]-base[1]), base[1]+s*(p[0]-base[0])+c*(p[1]-base[1]), 0.0]))
        elif kind == 'mirror':
            fields(value, 'axis_start axis_end copy')
            a, b = point(value['axis_start']), point(value['axis_end'])
            u = vector(a, b)
            ensure(length(u) > 1e-9 and type(value['copy']) is bool, '鏡射軸不能退化，copy須為布林值')
            def mirror(p):
                projection = dot(vector(a, p), u)/dot(u, u)
                return [2*(a[0]+projection*u[0])-p[0], 2*(a[1]+projection*u[1])-p[1], 0.0]
            add(map_geometry(row, mirror, mirrored=True), 'copy' if value['copy'] else 'modify')
        elif kind == 'array':
            fields(value, 'rows columns row_spacing column_spacing')
            count_rows, count_columns = value['rows'], value['columns']
            ensure(type(count_rows) is int and type(count_columns) is int and count_rows > 0 and count_columns > 0
                   and 2 <= count_rows*count_columns <= 100, '矩形陣列總數（含原圖元）需2–100')
            dy, dx = scalar(value['row_spacing']), scalar(value['column_spacing'])
            ensure((count_rows == 1 or dy != 0) and (count_columns == 1 or dx != 0), '陣列間距不可造成副本重疊')
            for r in range(count_rows):
                for c in range(count_columns):
                    if r or c:
                        add(map_geometry(row, lambda p,r=r,c=c: [p[0]+c*dx,p[1]+r*dy,0.0]), 'copy')
        elif kind == 'offset':
            fields(value, 'distance')
            distance = scalar(value['distance'])
            ensure(distance != 0 and row['kind'] in ('line', 'circle'), '偏移支援線與圓；distance須非零，線的正值為起點往終點左側，圓正值向外')
            if row['kind'] == 'line':
                u = vector(row['properties']['StartPoint'], row['properties']['EndPoint'])
                ensure(length(u) > 1e-9, '零長線不能偏移')
                delta = [-u[1]*distance/length(u),u[0]*distance/length(u)]
                updated = map_geometry(row, lambda p:[p[0]+delta[0],p[1]+delta[1],0.0])
            else:
                updated = copy.deepcopy(row)
                updated['properties']['Radius'] += distance
                ensure(updated['properties']['Radius'] > 1e-9, '偏移後圓半徑必須大於零')
            add(updated, 'copy')
        elif kind in ('trim', 'extend', 'fillet'):
            fields(value, 'boundary pick' if kind != 'fillet' else 'other_handle radius pick other_pick')
            other_handle = value['other_handle'] if kind == 'fillet' else value['boundary']
            ensure(other_handle in rows and other_handle != handle, '第二條線的Handle缺少或與主目標相同')
            other = geometry(rows[other_handle])
            ensure(row['kind'] == other['kind'] == 'line', '修剪／延伸／圓角目前只支援兩條直線')
            intersection, t, s = line_intersection(row, other)
            if kind != 'fillet':
                endpoint = closest_endpoint(row, point(value['pick']))
                if kind == 'trim':
                    ensure(1e-8 < t < 1-1e-8 and -1e-8 <= s <= 1+1e-8, '修剪需在目標線內，且位於邊界線段上有交點')
                else:
                    ensure((endpoint == 'StartPoint' and t < -1e-8 or endpoint == 'EndPoint' and t > 1+1e-8)
                           and -1e-8 <= s <= 1+1e-8, '延伸交點需在所選端點外，且位於邊界線段上')
                row['properties'][endpoint] = intersection
                add(row)
            else:
                ensure(other_handle not in used and other.get('editable') is True, '圓角第二條線不得在同批重複修改')
                ensure('fillet' in other.get('allowed_edits', []), '第二條線不支援圓角修改')
                ensure(not layers[other['properties']['Layer'].casefold()]['locked'], '圓角第二條線的圖層已鎖定')
                used.add(other_handle)
                radius = scalar(value['radius'])
                ensure(radius > 0, '圓角半徑必須大於零')
                keep1 = closest_endpoint(row, point(value['pick']))
                keep2 = closest_endpoint(other, point(value['other_pick']))
                u, v = vector(intersection,row['properties'][keep1]), vector(intersection,other['properties'][keep2])
                ensure(length(u)>1e-8 and length(v)>1e-8, '圓角保留側端點不得位於交點')
                u, v = [x/length(u) for x in u], [x/length(v) for x in v]
                theta = math.acos(max(-1,min(1,dot(u,v))))
                ensure(1e-6 < theta < math.pi-1e-6, '共線／近乎平行直線不能形成此圓角')
                tangent_distance = radius/math.tan(theta/2)
                ensure(tangent_distance < math.dist(intersection,row['properties'][keep1])-1e-7
                       and tangent_distance < math.dist(intersection,other['properties'][keep2])-1e-7, '圓角半徑過大，切點超過保留線段')
                tangent1 = [intersection[i]+tangent_distance*u[i] for i in range(2)]+[0.0]
                tangent2 = [intersection[i]+tangent_distance*v[i] for i in range(2)]+[0.0]
                bisector = [u[i]+v[i] for i in range(2)]
                center = [intersection[i]+bisector[i]/length(bisector)*radius/math.sin(theta/2) for i in range(2)]+[0.0]
                row['properties']['EndPoint' if keep1 == 'StartPoint' else 'StartPoint'] = tangent1
                other['properties']['EndPoint' if keep2 == 'StartPoint' else 'StartPoint'] = tangent2
                add(row)
                add(other, source=other_handle)
                start, end = (tangent2,tangent1) if cross(u,v)>0 else (tangent1,tangent2)
                arc = dict(kind='arc',properties={k:copy.deepcopy(v) for k,v in row['properties'].items() if k in ('Layer','Color','Linetype','LinetypeScale','Thickness','Normal')})
                arc['properties'].update(Center=center,Radius=radius,StartAngle=math.atan2(start[1]-center[1],start[0]-center[0])%math.tau,EndAngle=math.atan2(end[1]-center[1],end[0]-center[0])%math.tau)
                add(arc, 'arc', source=None)
        elif kind == 'layer':
            name = symbol(value)
            ensure(name.casefold() in layers and not layers[name.casefold()]['locked'], '目標圖層不存在或已鎖定；不自動解鎖或建立圖層')
            row['properties']['Layer'] = layers[name.casefold()]['name']
            add(row)
        ensure(len(plans) <= 1000, '本批展開後超過1000個幾何修改／新增圖元')
    for plan in plans:
        row=plan['updated']
        positions=[p for key,p in row['properties'].items() if key in ('StartPoint','EndPoint','Center')]
        positions.extend(row.get('geometry',{}).get('vertices',[]))
        ensure(all(all(math.isfinite(v) and abs(v)<=1_000_000 for v in p) for p in positions), '修改後座標超過一百萬單位範圍')
        if 'Radius' in row['properties']:
            ensure(0<row['properties']['Radius']<=1_000_000, '修改後半徑超出範圍')
    ensure(len(repr(plans).encode('utf-8'))<=2*1024*1024, '展開後幾何計畫超過2MiB，請拆成較小批次')
    return plans, used


def native_pairs(row):
    """只輸出本模組允許變更的DXF欄位，不序列化模型指定的原生資料。"""
    p, kind = row['properties'], row['kind']
    pairs = [[8,p['Layer']]]
    if kind == 'line': pairs.extend([[10,p['StartPoint']],[11,p['EndPoint']]])
    elif kind in ('circle','arc'):
        pairs.extend([[10,p['Center']],[40,p['Radius']]])
        if kind == 'arc': pairs.extend([[50,p['StartAngle']],[51,p['EndAngle']]])
    elif kind == 'polyline':
        pairs.extend([[90,len(row['geometry']['vertices'])],[70,1 if row['geometry']['closed'] else 0]])
        pairs.extend([[10,p[:2]] for p in row['geometry']['vertices']])
    return pairs


def literal(value):
    from cad_undo import lisp_string
    if isinstance(value,str): return lisp_string(value)
    if isinstance(value,list): return '('+' '.join(literal(v) for v in value)+')'
    ensure(type(value) in (int,float) and math.isfinite(value), '原生幾何資料不是有限數字')
    return repr(value)


def source(plans, output, nonce, doc):
    """原生端再核對原始幾何與圖層鎖；整批前置通過才開始第一項寫入。"""
    from cad_undo import lisp_string
    items=[]
    for index,plan in enumerate(plans):
        old = native_pairs(plan['old']) if plan['old'] else []
        updated = native_pairs(plan['updated'])
        if plan['mode']=='arc':
            p=plan['updated']['properties']
            updated=[[0,'ARC'],[100,'AcDbEntity'],[8,p['Layer']],[62,p['Color']],
                     [6,p['Linetype']],[48,p['LinetypeScale']],[100,'AcDbCircle'],[10,p['Center']],[40,p['Radius']],
                     [210,[0.,0.,1.]],[100,'AcDbArc'],[50,p['StartAngle']],[51,p['EndAngle']]]
        items.append([index,plan['source'] or '',plan['mode'],old,updated,plan['updated']['kind']])
    template = '''
(defun lm-geo-receipt (status / f)
  (setq f (open @@OUTPUT@@ "w"))
  (if f
    (progn
      (prin1 (list @@NONCE@@ status lm-geo-count lm-geo-handles) f)
      (close f))))
(defun lm-geo-error (message)
  (lm-geo-receipt "ERROR")
  (setq *error* lm-geo-old-error)
  (princ))
(defun lm-geo-values (data code / values pair)
  (foreach pair data
    (if (= (car pair) code) (setq values (cons (cdr pair) values))))
  (reverse values))
(defun lm-geo-pair-values (data code / values pair)
  (foreach pair data
    (if (= (car pair) code) (setq values (cons (cadr pair) values))))
  (reverse values))
(defun lm-geo-layer-open (name / data)
  (setq data (tblsearch "LAYER" name))
  (and data (= 0 (logand 4 (cdr (assoc 70 data))))))
(defun lm-geo-valid-item (item / entity data valid expected actual pair kind target)
  (setq target (cadr (assoc 8 (nth 4 item))))
  (setq valid (lm-geo-layer-open target))
  (if (/= (cadr item) "")
    (progn
      (setq entity (handent (cadr item)))
      (setq data (if entity (entget entity) nil))
      (setq kind (cdr (assoc 0 data)))
      (setq valid
        (and valid data
          (= kind (cond ((= (nth 5 item) "line") "LINE")
                        ((= (nth 5 item) "circle") "CIRCLE")
                        ((= (nth 5 item) "arc") "ARC")
                        ((= (nth 5 item) "polyline") "LWPOLYLINE")
                        (T "UNSUPPORTED")))
          (not (assoc 102 data)) (not (assoc 360 data))
          (not (assoc 420 data)) (not (assoc 430 data))
          (or (not (assoc 67 data)) (= 0 (cdr (assoc 67 data))))
          (lm-geo-layer-open (cdr (assoc 8 data)))))
      (foreach pair (nth 3 item)
        (setq expected (cadr pair) actual (cdr (assoc (car pair) data)))
        (if (and (= kind "LWPOLYLINE") (= (car pair) 70))
          (setq actual (logand 1 actual)))
        (if (not (and (= kind "LWPOLYLINE") (= (car pair) 10)))
          (if (not (equal actual expected 0.0000001)) (setq valid nil))))
      (if (= kind "LWPOLYLINE")
        (progn
          (if (not (equal (mapcar '(lambda (p) (list (car p) (cadr p))) (lm-geo-values data 10))
                          (lm-geo-pair-values (nth 3 item) 10) 0.0000001))
            (setq valid nil))
          (foreach pair data
            (if (and (= (car pair) 42) (/= (cdr pair) 0.0)) (setq valid nil)))))))
  valid)
(defun lm-geo-update (data pairs / pair result codes inserted replacement value code)
  (setq codes (mapcar 'car pairs))
  (foreach pair data
    (setq code (car pair))
    (if (member code codes)
      (if (not (member code inserted))
        (progn
          (foreach replacement pairs
            (if (= code (car replacement))
              (progn
                (setq value (cadr replacement))
                (if (and (= code 70) (= "LWPOLYLINE" (cdr (assoc 0 data))))
                  (setq value (+ value (- (cdr pair) (logand 1 (cdr pair))))))
                (setq result (cons (cons code value) result)))))
          (setq inserted (cons code inserted))))
      (setq result (cons pair result))))
  (foreach replacement pairs
    (if (not (member (car replacement) inserted))
      (setq result (cons (cons (car replacement) (cadr replacement)) result))))
  (reverse result))
(defun lm-geo-write (item / entity data output pairs pair result)
  (setq entity (if (/= (cadr item) "") (handent (cadr item)) nil))
  (setq data (if entity (entget entity) nil))
  (if (= (nth 2 item) "arc")
    (setq output (mapcar '(lambda (p) (cons (car p) (cadr p))) (nth 4 item)))
    (setq output (lm-geo-update data (nth 4 item))))
  (if (= (nth 2 item) "modify")
    (if (entmod output) (progn (entupd entity) (setq result entity)))
    (progn
      (foreach pair output
        (if (not (member (car pair) '(-1 5 330))) (setq pairs (cons pair pairs))))
      (if (entmake (reverse pairs)) (setq result (entlast)))))
  result)
(defun lm-geo-run (/ plan valid item created ok lm-geo-old-error lm-geo-count lm-geo-handles)
  (setq lm-geo-old-error *error* lm-geo-count 0 lm-geo-handles "")
  (setq *error* lm-geo-error)
  (setq plan '@@PLAN@@)
  (setq valid
    (and (= (getvar "DWGNAME") @@NAME@@)
         (= (getvar "DWGPREFIX") @@PREFIX@@)
         (= (getvar "INSUNITS") @@UNITS@@)
         (/= 0 (logand 8 (getvar "UNDOCTL")))))
  (foreach item plan
    (if (not (lm-geo-valid-item item))
      (progn (setq valid nil)
        (setq lm-geo-handles (strcat lm-geo-handles (itoa (car item)) ":0;")))))
  (if valid
    (progn
      (setq ok T)
      (foreach item plan
        (if ok
          (progn
            (setq created (lm-geo-write item))
            (if created
              (progn
                (setq lm-geo-count (1+ lm-geo-count))
                (setq lm-geo-handles
                  (strcat lm-geo-handles (itoa (car item)) ":"
                          (cdr (assoc 5 (entget created))) ";")))
              (setq ok nil)))))
      (lm-geo-receipt (if ok "OK" "ERROR")))
    (lm-geo-receipt "PRECONDITION"))
  (setq *error* lm-geo-old-error)
  (princ))
(lm-geo-run)
(princ)
'''
    replacements=dict(OUTPUT=lisp_string(output.as_posix()),NONCE=lisp_string(nonce),PLAN=literal(items),
                      NAME=lisp_string(str(doc.GetVariable('DWGNAME'))),PREFIX=lisp_string(str(doc.GetVariable('DWGPREFIX'))),UNITS=literal(doc.GetVariable('INSUNITS')))
    return re.sub(r'@@(OUTPUT|NONCE|PLAN|NAME|PREFIX|UNITS)@@',lambda m:replacements[m[1]],template)


def apply(app, doc, plans, report, guard, legacy):
    """固定模板由可信 runtime 載入；未知結果保留腳本／回條，不自動復原或重播。"""
    from cad_undo import begin_group
    # 使用既有固定匯出器的載入方式；不新增可接受模型路徑的入口。
    folder=Path(tempfile.mkdtemp(prefix='LMAI-geometry-'))
    output, script=folder/'receipt.txt',folder/'geometry.lsp'
    nonce=uuid.uuid4().hex
    import ctypes
    encoding='cp'+str(ctypes.windll.kernel32.GetACP())
    script.write_text(source(plans,output,nonce,doc),encoding=encoding)
    finish=begin_group(app,doc,report,guard,legacy=legacy)
    try:
        guard()
        report.update(step='native_geometry_edit',drawing_started=True)
        if legacy:
            app.LoadLISP(str(script))
        else:
            from cad_undo import lisp_string
            doc.SendCommand('(load '+lisp_string(script.as_posix())+')\n')
        import pythoncom
        deadline=time.monotonic()+60
        while True:
            guard()
            if output.exists():
                match=re.fullmatch(r'\("'+nonce+r'" "(OK|ERROR|PRECONDITION)" ([0-9]+) "([0-9A-Fa-f:;]*)"\)\s*',output.read_text(encoding='ascii'))
                if match: break
            if time.monotonic()>=deadline: raise TimeoutError('幾何修改未取得完整回條，結果未知，不重送')
            pythoncom.PumpWaitingMessages()
            time.sleep(.05)
        status,count,raw=match[1],int(match[2]),match[3]
        report['native_completed_actions']=count
        if status=='PRECONDITION': report['invalid_geometry_action_indices']=raw
        handles={}
        for pair in filter(None,raw.split(';')):
            index,handle=pair.split(':')
            ensure(index.isdigit() and int(index)<len(plans) and int(index) not in handles, '幾何修改回條索引不符')
            handles[int(index)]=handle.upper()
        if status=='PRECONDITION' and count==0: report['drawing_started']=False
        ensure(status=='OK' and count==len(plans) and set(handles)==set(range(len(plans))), '原生幾何修改未完成：'+status+'；不自動復原或重送')
        report['geometry_action_handles']=[handles[i] for i in range(len(plans))]
        report['completed_operations']=len({p['operation'] for p in plans})
        return handles,finish
    except BaseException:
        finish()
        raise


def execute(app, doc, legacy, request, report, api, entities, before, output, guard):
    """既有圖面完整核對／一次Undo，與原先修改器保持同樣的不儲存契約。"""
    from cad_existing import iter_entities, snapshot
    ensure(not request['operations'], '幾何修改請與新增／指令分批，避免兩種Undo路徑混合')
    rows = {row['handle']:row for row in before}
    layers = {str(layer.Name).casefold():dict(name=str(layer.Name),locked=bool(layer.Lock)) for layer in iter_entities(doc.Layers)}
    plans, used = prepare(request['edits'], rows, layers)
    ensure(plans, '沒有可執行的幾何修改')
    def check_document():
        if guard: guard()
        ensure(app.ActiveDocument._oleobj_ == doc._oleobj_, '作用中CAD文件已切換，停止修改')
    handles, finish = apply(app,doc,plans,report,check_document,legacy)
    expected = copy.deepcopy(rows)
    modified={plan['source'] for plan in plans if plan['mode']=='modify'}
    try:
        for index,plan in enumerate(plans):
            row = copy.deepcopy(plan['updated'])
            row['handle'] = handles[index]
            expected[handles[index]] = row
        def verify(target):
            from cad_existing import comparison
            actual, actual_rows = snapshot(target,api)
            described = {row['handle']:row for row in actual_rows}
            ensure(set(actual)==set(expected), '幾何修改後Handle集合不符，不能宣稱完成')
            for handle,row in expected.items():
                for field,value in row['properties'].items():
                    available = described[handle]['properties']
                    observed = available[field] if field in available else getattr(actual[handle],field)
                    ensure(api.equal(observed,value), '幾何屬性核對失敗：'+handle+'.'+field)
                if 'geometry' in row:
                    observed = described[handle].get('geometry',{})
                    expected_geometry = row['geometry']
                    ensure(observed.get('closed')==expected_geometry['closed']
                           and observed.get('straight_segments') is True
                           and len(observed.get('vertices',[]))==len(expected_geometry['vertices'])
                           and all(api.equal(a,b) for a,b in zip(observed['vertices'],expected_geometry['vertices'])), '多段線修改後頂點核對失敗')
                if handle in rows and handle not in modified and rows[handle].get('native_revision'):
                    ensure(described[handle].get('native_revision') == rows[handle]['native_revision'], '未指定圖元的原生資料改變：'+handle)
            return dict(handles_verified=len(actual),available_properties_verified=True,comparison=comparison(before,actual_rows),
                        unavailable_properties_not_verified=True,geometry_actions_verified=len(plans))
        report['step']='verify_before_save'
        check_document()
        doc.Regen() if legacy else doc.Regen(1)
        before_save=verify(doc)
    finally:
        finish()
    added=[handles[i] for i,p in enumerate(plans) if p['mode']!='modify']
    if output is None:
        return dict(status='EDITED_UNSAVED',saved=False,verification=before_save,modified_handles=sorted(modified),source_handles=sorted(used),
                    added_handles=added,added_entities=len(added),visual_check_required=True,
                    advice='本輪固定幾何修改完成，未儲存、未關閉；可在CAD以一次Undo還原。')
    report['step']='save'
    doc.Save()
    report['saved']=True
    doc.Close(False)
    report['step']='reopen'
    reopened=app.Documents.Open(str(output))
    ensure(Path(str(reopened.FullName)).resolve()==output.resolve(), '重開修改副本的路徑不符')
    after_save=verify(reopened)
    reopened.Close(False)
    return dict(status='DATA_VERIFIED_VISUAL_PENDING',before_save=before_save,after_reopen=after_save,
                modified_handles=sorted(modified),source_handles=sorted(used),added_handles=added,added_entities=len(added),visual_check_required=True)
