"""既有普通圓圖塊的有界讀寫。

修改定義會影響每個引用，因此只接受同一批明確列出全部 ModelSpace 引用。
巢狀／配置空間／外部引用、動態圖塊與非圓成員不在此入口支援範圍。
不分解、不重建圖塊，不自動儲存目前文件；失敗保留原生診斷與 Undo 群組。
"""
import copy
import math
from contextlib import ExitStack
from pathlib import Path

from cad_existing import describe, iter_entities, optional

MAX_DEFINITIONS = 128
MAX_MEMBERS = 32
MAX_REFERENCE_SCAN = 20000


def annotate(doc, entities, rows, api, guard=None):
    """在既有索引附上可核對的局部圓資料，未支援的圖塊仍明示唯讀原因。"""
    cache = {}
    references = {}
    for row in rows:
        if row['kind'] == 'blockreference':
            name = str(row['properties'].get('Name', ''))
            references.setdefault(name.casefold(), []).append(row['handle'])
    for row in rows:
        if row['kind'] != 'blockreference':
            continue
        if guard:
            guard()
        name = str(row['properties'].get('Name', ''))
        key = name.casefold()
        row['editable'], row['allowed_edits'] = False, []
        try:
            api.require(name and not name.startswith('*') and '|' not in name,
                        '匿名、動態或外部圖塊不支援此入口')
            if key not in cache:
                api.require(len(cache) < MAX_DEFINITIONS, '圖塊定義詳讀超過128個上限')
                block = doc.Blocks.Item(name)
                api.require(optional(block, 'IsXRef') is False, '外部引用或無法判定的圖塊不支援')
                api.require(not optional(block, 'IsDynamicBlock'), '動態圖塊不支援')
                count = optional(block, 'Count')
                api.require(type(count) is int and 1 <= count <= MAX_MEMBERS,
                            '此入口僅支援1–32個圓成員的普通圖塊')
                members = [describe(member, api) for member in iter_entities(block)]
                api.require(len(members) == count and all(m['kind'] == 'circle' for m in members),
                            '此入口僅支援直接圓成員，含其他／巢狀圖元的圖塊維持唯讀')
                for member in members:
                    props = member['properties']
                    api.require(all(field in props for field in ('Radius', 'Color', 'Center', 'Normal', 'Layer')),
                                '圓成員缺少完整的可核對屬性')
                    api.require(props['Radius'] > 0 and api.equal(props['Normal'], [0, 0, 1]),
                                '只支援平面圓成員')
                cache[key] = {'name': name, 'members': members,
                              'model_reference_handles': sorted(references[key]),
                              'coordinates': 'block-local; diameter parameter uses displayed drawing units',
                              'scope': 'shared definition; all references must be explicitly included'}
            info = copy.deepcopy(cache[key])
            ref = entities[row['handle']]
            api.require(not optional(ref, 'IsDynamicBlock'), '動態圖塊引用不支援')
            scales = [optional(ref, field) for field in ('XScaleFactor', 'YScaleFactor', 'ZScaleFactor')]
            api.require(all(type(v) in (int, float) and math.isfinite(v) and v > 0 for v in scales)
                        and api.equal(scales, [scales[0]] * 3), '只支援正向等比例圖塊引用')
            info['scale'] = scales[0]
            row['block'] = info
            row['editable'], row['allowed_edits'] = True, ['block_circle']
        except Exception as error:
            # 可讀摘要仍可交付；不能把探測失敗解讀成可寫。
            row['block'] = {'name': name, 'read_only_reason': str(error)[:400]}


def reference_scope(doc, names, api, guard):
    """寫入前掃描定義／配置引用，防止修改未被使用者選中的其他實例。"""
    result = {name: [] for name in names}
    blocks = doc.Blocks
    api.require(type(blocks.Count) is int and blocks.Count <= 1024, '圖塊集合超過安全核對上限')
    scanned = 0
    # 2008的Blocks.Item('*Model_Space')列舉為空，ModelSpace卻有圖元。
    # 主空間一定走文件集合；配置空間另讀，與Blocks的結果按Handle去重。
    collections = [('*model_space', doc.ModelSpace), ('*paper_space', doc.PaperSpace)]
    collections += [(str(block.Name).casefold(), block) for block in iter_entities(blocks)
                    if str(block.Name).casefold() != '*model_space']
    for owner, block in collections:
        guard()
        count = optional(block, 'Count')
        api.require(type(count) is int and count >= 0, '無法核對全部圖塊引用')
        scanned += count
        api.require(scanned <= MAX_REFERENCE_SCAN, '全圖引用核對超過20000圖元，尚未修改')
        seen = 0
        for entity in iter_entities(block):
            seen += 1
            native_type = str(optional(entity, 'ObjectName') or optional(entity, 'EntityName') or '').lower()
            api.require(bool(native_type), '有無法辨識的圖元，不能確認引用範圍')
            if 'blockreference' in native_type or 'insert' in native_type:
                name = str(entity.Name).casefold()
                if name in result:
                    reference = (owner, str(entity.Handle).upper())
                    if reference not in result[name]:
                        result[name].append(reference)
        api.require(seen == count, '掃描期間圖塊集合已變更')
    return result


def prepare(doc, request, before, api, guard):
    """整批先驗證；同一定義的參數必須一致，才合併成一次原生成員修改。"""
    edits = request['edits']
    api.require(edits and not request['operations'] and len(edits) <= 100,
                'block_circle需獨立一批，operations為[]')
    rows = {row['handle']: row for row in before}
    groups, used = {}, set()
    for edit in edits:
        api.require(set(edit) == {'handle', 'kind', 'value'} and edit['kind'] == 'block_circle',
                    '圓圖塊與其他修改不能混用')
        handle, value = edit['handle'], edit['value']
        api.require(handle in rows and handle not in used, '圖塊Handle不存在或重複')
        used.add(handle)
        row = rows[handle]
        api.require('block_circle' in row.get('allowed_edits', []), '此圖塊未開放圓成員修改')
        api.require(isinstance(value, dict) and set(value) == {'member_handle', 'diameter', 'color'},
                    'value需member_handle、diameter、color；只取自已讀圖塊資料')
        info = row['block']
        member = next((m for m in info['members'] if m['handle'] == value['member_handle']), None)
        api.require(member is not None, 'member_handle不屬於所選圖塊')
        diameter = api.number(value['diameter'])
        api.require(0 < diameter <= 1e12 and type(value['color']) is int and 1 <= value['color'] <= 255,
                    '直徑需為正有限數，color需ACI 1–255（紅1、綠3）')
        radius = diameter / (2 * info['scale'])
        api.require(math.isfinite(radius) and 0 < radius <= 1e12, '換算後局部半徑超出範圍')
        for layer in (row['properties']['Layer'], member['properties']['Layer']):
            api.require(not bool(doc.Layers.Item(layer).Lock), '圖塊引用或圓成員的圖層已鎖定')
        key = info['name'].casefold()
        plan = (member['handle'], radius, value['color'])
        if key in groups:
            api.require(groups[key]['plan'] == plan, '共用圖塊參數不一致；不能給各引用不同直徑／顏色')
            groups[key]['handles'].add(handle)
        else:
            groups[key] = {'plan': plan, 'member': member, 'handles': {handle}, 'info': info}
    scope = reference_scope(doc, groups, api, guard)
    for name, group in groups.items():
        expected = {('*model_space', h) for h in group['handles']}
        api.require(set(scope[name]) == expected and len(scope[name]) == len(expected),
                    '修改共用圖塊會影響未列入的引用（可能位於模型、配置或巢狀圖塊）；請列齊全部ModelSpace引用，否則不修改')
    return groups


def execute(app, doc, legacy, request, report, api, entities, before, output=None, guard=None):
    """保持一次Undo涵蓋修改和讀回；副本另作儲存／重開核對。"""
    from cad_existing import snapshot, comparison
    from cad_undo import apply as apply_legacy, begin_group

    if guard is None:
        def guard():
            api.require(app.ActiveDocument._oleobj_ == doc._oleobj_, '作用文件已切換，停止修改')
    guard()
    groups = prepare(doc, request, before, api, guard)
    expected = copy.deepcopy(before)
    native_edits, member_rows, member_objects = [], {}, {}
    for group in groups.values():
        handle, radius, color = group['plan']
        member_rows[handle] = group['member']
        # 2008不能對定義內成員使用HandleToObject；從已核對的定義集合定位。
        block = doc.Blocks.Item(group['info']['name'])
        member_objects[handle] = next(m for m in iter_entities(block) if str(m.Handle).upper() == handle)
        native_edits.extend([dict(handle=handle, kind='radius', value=radius),
                             dict(handle=handle, kind='color', value=color)])
        for row in expected:
            if row['handle'] in group['handles']:
                for member in row['block']['members']:
                    if member['handle'] == handle:
                        member['properties'].update(Radius=radius, Color=color)

    def verify(target):
        _, actual = snapshot(target, api)
        by_handle = {row['handle']: row for row in actual}
        api.require(set(by_handle) == {r['handle'] for r in expected}, '修改後主圖元集合不同')
        for row in expected:
            observed = by_handle[row['handle']]
            for key in ('properties', 'geometry', 'native_revision', 'block'):
                # api.equal支援數值／座標，結構遞迴在此保留欄位完整性。
                api.require(same(row.get(key), observed.get(key), api),
                            '圖塊修改後核對失敗：' + row['handle'] + '.' + key)
        return {'handles_verified': len(actual), 'block_members_verified': True,
                'comparison': comparison(before, actual), 'shared_definitions': len(groups)}

    with ExitStack() as cleanup:
        guard()
        if legacy:
            cleanup.callback(apply_legacy(app, doc, native_edits, member_rows, report, guard))
        else:
            cleanup.callback(begin_group(app, doc, report, guard))
            for group in groups.values():
                guard()
                handle, radius, color = group['plan']
                report['drawing_started'] = True
                member_objects[handle].Radius = radius
                member_objects[handle].Color = color
                report['completed_operations'] = report.get('completed_operations', 0) + 2
        guard()
        doc.Regen() if legacy else doc.Regen(1)
        verification = verify(doc)
    modified = sorted(edit['handle'] for edit in request['edits'])
    if output is None:
        return {'status': 'EDITED_UNSAVED', 'saved': False, 'verification': verification,
                'modified_handles': modified, 'added_entities': 0, 'visual_check_required': True,
                'advice': '共用圓圖塊已修改並讀回；位置與其他圖元核對未變。尚未儲存，可用一次Undo復原。尺寸標註關聯仍需確認。'}
    report['step'] = 'save'
    doc.Save()
    report['saved'] = True
    doc.Close(False)
    reopened = app.Documents.Open(str(output))
    api.require(Path(str(reopened.FullName)).resolve() == Path(output).resolve(), '重開副本路徑不同')
    after_save = verify(reopened)
    reopened.Close(False)
    return {'status': 'DATA_VERIFIED_VISUAL_PENDING', 'before_save': verification,
            'after_reopen': after_save, 'modified_handles': modified, 'visual_check_required': True}


def same(a, b, api):
    """遞迴核對完整可讀結構，數值沿用CAD既有浮點容差。"""
    if isinstance(a, dict):
        return isinstance(b, dict) and a.keys() == b.keys() and all(same(a[k], b[k], api) for k in a)
    if isinstance(a, list):
        return isinstance(b, list) and len(a) == len(b) and all(same(x, y, api) for x, y in zip(a, b))
    return api.equal(a, b) if type(a) in (int, float) and type(b) in (int, float) else a == b
