"""有界的二維幾何：先計算／驗證，再使用固定 CAD 介面建立原生圖元。

新建圖面與既有圖元修改共用幾何計算；所有角度輸入為度，原生 API 使用弧度。
不接受指令、腳本、外部圖塊檔或使用者指定的 COM 成員名稱。
"""
import copy
import math
import re

EXTRA_KINDS = {'arc', 'polyline', 'radius_dimension', 'angular_dimension',
               'layer', 'block_definition', 'block_reference'}
EDIT_KINDS = {'copy', 'rotate', 'mirror', 'array', 'offset', 'trim', 'extend', 'fillet', 'layer'}


def symbol(value):
    """只接受圖面內的普通名稱，杜絕外部檔名／路徑及保留的匿名圖塊。"""
    from worker import require
    require(isinstance(value, str) and 1 <= len(value) <= 80
            and not any(c in value for c in '<>/\\":;?*|=,`\x00')
            and not value.startswith(' ') and not value.endswith(' ')
            and not any(ord(c) < 32 for c in value), '圖層／圖塊名稱不合法')
    return value


def angle(value):
    from worker import number
    return math.radians(number(value)) % math.tau


def prepare_extra(source):
    """僅識別新增的固定操作；None 交還原有驗證器。"""
    from worker import require, number, point_value, prepare_line_style, prepare
    kind = source.get('kind')
    if kind not in EXTRA_KINDS:
        return None
    fields = {'arc': 'center radius start_angle end_angle', 'polyline': 'points closed',
              'radius_dimension': 'center chord leader_length',
              'angular_dimension': 'vertex start end location', 'layer': 'name linetype',
              'block_definition': 'name base entities', 'block_reference': 'name position scale rotation'}
    optional = {'layer'} if kind not in ('layer', 'block_definition') else set()
    if kind in ('arc', 'polyline'):
        optional.add('line_style')
    required = {'kind'} | set(fields[kind].split())
    if kind != 'block_definition':
        required.add('color')
    require(required <= source.keys() <= required | optional, '新增幾何的欄位缺漏或包含未知欄位')
    op = copy.deepcopy(source)
    if 'color' in op:
        require(type(op['color']) is int and 1 <= op['color'] <= 255, '顏色需為 ACI 1–255')
    if 'layer' in op:
        op['layer'] = symbol(op['layer'])
    if 'line_style' in op:
        op['line_style'] = prepare_line_style(op['line_style'])
    if 'name' in op:
        op['name'] = symbol(op['name'])
    for key in ('center', 'chord', 'vertex', 'start', 'end', 'location', 'base', 'position'):
        if key in op:
            op[key] = point_value(op[key])
    for key in ('radius', 'leader_length', 'scale'):
        if key in op:
            op[key] = number(op[key])
            require(op[key] > 0, key + ' 必須大於零')
    if kind == 'arc':
        op['start_angle'], op['end_angle'] = angle(op['start_angle']), angle(op['end_angle'])
        require(not math.isclose((op['end_angle']-op['start_angle']) % math.tau, 0, abs_tol=1e-9), '圓弧角度不能相同，整圓請用circle')
    elif kind == 'polyline':
        require(isinstance(op['points'], list) and 2 <= len(op['points']) <= 512
                and type(op['closed']) is bool, '多段線需2–512個不重複端點與closed布林值')
        op['points'] = [point_value(p) for p in op['points']]
        require(all(a != b for a, b in zip(op['points'], op['points'][1:])), '多段線包含零長線段')
        require(not op['closed'] or len(op['points']) >= 3 and op['points'][0] != op['points'][-1], '封閉多段線至少3點，不重複首點')
    elif kind == 'radius_dimension':
        require(op['center'] != op['chord'], '半徑尺寸的中心與圓上點不能相同')
    elif kind == 'angular_dimension':
        a, b = vector(op['vertex'], op['start']), vector(op['vertex'], op['end'])
        require(length(a) > 1e-9 and length(b) > 1e-9 and abs(cross(a, b)) > 1e-9, '角度尺寸的兩條射線不能退化或共線')
        require(op['location'] != op['vertex'], '角度尺寸位置不能在頂點')
        first, second = math.atan2(a[1],a[0]), math.atan2(b[1],b[0])
        sweep=(second-first)%math.tau
        if sweep>math.pi:
            # 原生三點角度尺寸會把小角的兩條射線按逆時針排列。
            # 建立前先以相同方式正規化，讓預期端點仍取自輸入幾何，
            # 避免順時針輸入建立成功後，僅因原生交換端點而被誤拒絕。
            op['start'], op['end'] = op['end'], op['start']
            first,second=second,first
            sweep=(second-first)%math.tau
        location=vector(op['vertex'],op['location'])
        offset=(math.atan2(location[1],location[0])-first)%math.tau
        require(1e-9<offset<sweep-1e-9, '角度尺寸目前只支援小於180度的小角，location需在小角區域內')
    elif kind == 'layer':
        op['linetype'] = prepare_line_style(dict(name=op['linetype'], scale=1))['name']
        require(op['linetype'] not in ('BYLAYER', 'BYBLOCK'), '圖層線型不可為BYLAYER或BYBLOCK')
    elif kind == 'block_definition':
        require(isinstance(op['entities'], list) and 1 <= len(op['entities']) <= 50, '每個圖塊定義需1–50個基本圖元')
        require(all(isinstance(p, dict) and p.get('kind') in ('line', 'circle', 'arc', 'polyline') for p in op['entities']), '圖塊定義只接受線／圓／圓弧／直線多段線，不接受巢狀或外部圖塊')
        op['entities'] = prepare(op['entities'])
    elif kind == 'block_reference':
        op['rotation'] = angle(op['rotation'])
    return op


def draw_extra(drawing, op):
    """只建立已驗證的原生圖元，預期值取自計畫，不以讀到的結果代替預期。"""
    from worker import VARIANT, pythoncom, require
    kind, model = op['kind'], drawing.model
    color = op.get('color', 7)
    if kind == 'arc':
        obj = model.AddArc(drawing.point(op['center']), op['radius'], op['start_angle'], op['end_angle'])
        drawing.remember(obj, color, Center=op['center'], Radius=op['radius'], StartAngle=op['start_angle'], EndAngle=op['end_angle'])
    elif kind == 'polyline':
        values = drawing.points(op['points']) if drawing.legacy else VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, tuple(v for p in op['points'] for v in p[:2]))
        obj = model.AddLightWeightPolyline(values)
        obj.Closed = op['closed']
        drawing.remember(obj, color, Closed=op['closed'])
        drawing.expected[-1]['geometry'] = dict(vertices=op['points'], closed=op['closed'], coordinates='WCS', straight_segments=True)
    elif kind in ('radius_dimension', 'angular_dimension'):
        style = drawing.dimension_style()
        if kind == 'radius_dimension':
            obj = model.AddDimRadial(drawing.point(op['center']), drawing.point(op['chord']), op['leader_length'])
            measurement = math.dist(op['center'], op['chord'])
        else:
            obj = model.AddDim3PointAngular(*(drawing.point(op[k]) for k in ('vertex', 'start', 'end', 'location')))
            a, b = vector(op['vertex'], op['start']), vector(op['vertex'], op['end'])
            measurement = math.acos(max(-1, min(1, dot(a, b)/(length(a)*length(b)))))
        obj.StyleName = style
        drawing.remember(obj, color, StyleName=style)
        drawing.expected[-1]['measurement'] = measurement
        drawing.expected[-1]['dimension_geometry'] = (dict(kind='radius',center=op['center'],chord=op['chord'],measurement=measurement)
            if kind=='radius_dimension' else dict(kind='angular',vertex=op['vertex'],start=op['start'],end=op['end'],measurement=measurement))
    elif kind == 'layer':
        # 定義已在整批 prepare_styles 建立及讀回，不變更目前作用中圖層。
        pass
    elif kind == 'block_definition':
        require(not any(str(b.Name).casefold() == op['name'].casefold() for b in __import__('cad_existing').iter_entities(drawing.doc.Blocks)), '圖塊名稱已存在，不覆寫既有定義')
        block = drawing.doc.Blocks.Add(drawing.point(op['base']), op['name'])
        child = drawing.__class__.__new__(drawing.__class__)
        child.app, child.doc, child.model, child.legacy, child.expected = drawing.app, drawing.doc, block, drawing.legacy, []
        child.report = drawing.report
        child.linetype_names = getattr(drawing, 'linetype_names', {})
        for entity in op['entities']:
            expected_start = len(child.expected)
            child.last_created_entity = None
            child.draw_entity(entity)
            obj = child.last_created_entity
            require(len(child.expected) == expected_start+1 and obj is not None
                    and str(obj.Handle).upper() == child.expected[-1]['handle'],
                    '圖塊基本圖元的原生參照與預期Handle不符')
            if 'line_style' in entity:
                child.expected[-1]['properties'].update(child.apply_line_style(obj,entity['line_style']))
            if 'layer' in entity:
                obj.Layer = entity['layer']
                child.expected[-1]['properties']['Layer'] = entity['layer']
        drawing.block_expected = getattr(drawing, 'block_expected', {})
        drawing.block_expected[op['name']] = child.expected
    elif kind == 'block_reference':
        require(op['name'] in getattr(drawing, 'block_expected', {}), '只允許插入本輪明確建立的圖塊，不讀取外部DWG')
        obj = model.InsertBlock(drawing.point(op['position']), op['name'], op['scale'], op['scale'], op['scale'], op['rotation'])
        drawing.remember(obj, color, InsertionPoint=op['position'], Rotation=op['rotation'])
        drawing.expected[-1]['block_name'] = op['name']
        drawing.expected[-1]['block_scale'] = op['scale']
    else:
        return False
    return True


def prepare_layers(drawing, operations):
    """整批圖層與圖塊子圖元先檢查；不解鎖、不更名、不刪除使用者圖層。"""
    from worker import require
    from cad_existing import iter_entities
    definitions = [op for op in operations if op['kind'] == 'layer']
    shapes = list(operations) + [child for op in operations if op['kind']=='block_definition' for child in op['entities']]
    if not definitions and not any('layer' in op for op in shapes):
        return
    layers = {str(item.Name).casefold(): item for item in iter_entities(drawing.doc.Layers)}
    require(len({op['name'].casefold() for op in definitions}) == len(definitions), '同批圖層名稱重複')
    drawing.layer_expected = getattr(drawing, 'layer_expected', {})
    for op in definitions:
        key = op['name'].casefold()
        layer = layers.get(key)
        require(layer is None, '圖層名稱已存在；不以新建指令覆寫既有圖層')
        layer = drawing.doc.Layers.Add(op['name'])
        layer.Color = op['color']
        layer.Linetype = drawing.linetype_names[op['linetype']]
        drawing.layer_expected[op['name']] = dict(Color=op['color'], Linetype=str(layer.Linetype))
        layers[key] = layer
    for op in shapes:
        if 'layer' in op:
            require(op['layer'].casefold() in layers and not bool(layers[op['layer'].casefold()].Lock), '圖元圖層不存在或已鎖定')


def verify_extra(drawing, doc, entities, rows):
    from worker import require, equal
    from cad_existing import iter_entities, describe
    def same_vertices(actual, expected):
        """沿用CAD數值容差，避免原生16位小數輸出造成極小浮點差而誤拒絕。"""
        return (isinstance(actual,dict) and actual.get('closed')==expected['closed']
                and actual.get('straight_segments') is True and actual.get('coordinates')=='WCS'
                and len(actual.get('vertices',[]))==len(expected['vertices'])
                and all(equal(a,b) for a,b in zip(actual['vertices'],expected['vertices'])))
    described = {row['handle']:row for row in rows} if rows and isinstance(rows[0],dict) else {}
    for row in drawing.expected:
        if 'dimension_geometry' in row:
            actual=described.get(row['handle'],{}).get('dimension_geometry',{})
            expected=row['dimension_geometry']
            require(set(actual)==set(expected) and all(equal(actual[key],value) for key,value in expected.items()), '原生尺寸端點／量測值核對失敗')
        if 'geometry' in row:
            actual = described.get(row['handle']) or describe(entities[row['handle']], __import__('worker'))
            require(same_vertices(actual.get('geometry'),row['geometry']), '原生多段線頂點／封閉狀態核對失敗')
        if 'block_scale' in row:
            obj = entities[row['handle']]
            # iCAD2008 的 INSERT.Name 會轉成大寫；圖塊符號名稱不分大小寫。
            # 只對本輪自建圖塊名稱採此比較，座標、比例及其他屬性仍逐值核對。
            require(str(obj.Name).casefold()==row['block_name'].casefold(), '原生圖塊名稱核對失敗')
            for field in ('XScaleFactor','YScaleFactor','ZScaleFactor'):
                require(equal(getattr(obj, field),row['block_scale']), '圖塊比例核對失敗')
    for name, properties in getattr(drawing,'layer_expected',{}).items():
        layer = doc.Layers.Item(name)
        for key,value in properties.items():
            require(equal(getattr(layer,key),value), '圖層定義核對失敗：'+name+'.'+key)
    for name, expected in getattr(drawing,'block_expected',{}).items():
        block = doc.Blocks.Item(name)
        actual = {str(entity.Handle).upper():entity for entity in iter_entities(block)}
        require(set(actual) == {row['handle'] for row in expected}, '圖塊內圖元集合不符')
        for row in expected:
            for key,value in row['properties'].items():
                require(equal(getattr(actual[row['handle']],key),value), '圖塊內圖元屬性不符：'+key)
            if 'geometry' in row:
                require(same_vertices(describe(actual[row['handle']],__import__('worker')).get('geometry'),row['geometry']), '圖塊內多段線核對失敗')


def vector(a, b): return [b[0]-a[0], b[1]-a[1]]
def dot(a, b): return a[0]*b[0]+a[1]*b[1]
def cross(a, b): return a[0]*b[1]-a[1]*b[0]
def length(a): return math.hypot(*a)
