"""雙版CAD實機回歸；只建立獨立測試圖，不改使用者原圖。

以固定runtime執行，參數為新測試目錄。失敗資料保留，不覆寫既有證據。
"""
import copy
import hashlib
import json
from pathlib import Path
import sys
import time


def main():
    root = Path(sys.argv[1]).resolve()
    root.mkdir(parents=True, exist_ok=False)
    project = Path(__file__).resolve().parents[1]
    stage = Path((project / '.build/cad-runtime-stage.txt').read_text().strip())
    sys.path.insert(0, str(stage))
    import worker
    worker.BULK_ENABLED = True
    import pythoncom
    import win32com.client
    from cad_existing import iter_entities, snapshot
    from cad_block_circles import same
    pythoncom.CoInitialize()
    results = []
    try:
        for year, version in [('2008', 'Icad.Application.1'), ('2022', 'GStarCAD.Application.22')]:
            if len(sys.argv) > 2 and year != sys.argv[2]:
                continue
            output = root / (year + '.dwg')
            report = {}
            ops = [dict(kind='block_definition', name='HOLE_C', base=[0, 0],
                        entities=[dict(kind='circle', center=[0, 0], radius=5, color=1)])]
            ops += [dict(kind='block_reference', name='HOLE_C', position=pos, scale=1, rotation=0, color=7)
                    for pos in ([15, 15], [105, 15], [15, 65], [105, 65])]
            ops += [dict(kind='line', start=[0, 0], end=[120, 80], color=4)]
            report.update(worker.execute(dict(action='draw', version=version, output=str(output), operations=ops,
                                              launch_if_missing=False, wait_for_ready=False), report))
            (root / (year + '-draw.json')).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
            app = win32com.client.GetActiveObject(version)
            previous = app.ActiveDocument
            doc = app.Documents.Open(str(output))
            original_hash = hashlib.sha256(output.read_bytes()).hexdigest()

            def read():
                return worker.execute(dict(action='active_snapshot', version=version,
                                           launch_if_missing=False, wait_for_ready=False), {})

            before = read()
            (root / (year + '-before.json')).write_text(json.dumps(before, ensure_ascii=False, indent=2), encoding='utf-8')
            refs = [row for row in before['entities'] if row['kind'] == 'blockreference']
            assert len(refs) == 4 and all(row['allowed_edits'] == ['block_circle'] for row in refs), refs
            edits = [dict(handle=row['handle'], kind='block_circle', value={
                'member_handle': row['block']['members'][0]['handle'], 'diameter': 30, 'color': 3}) for row in refs]
            request = dict(action='active_edit', version=version, document=before['document'], revision=before['revision'],
                           edits=edits, operations=[], launch_if_missing=False, wait_for_ready=False)
            rejected = []
            for name, bad in [('subset', dict(request, edits=edits[:1])),
                              ('stale', dict(request, revision='stale')),
                              ('bad_member', dict(request, edits=copy.deepcopy(edits)))]:
                if name == 'bad_member':
                    bad['edits'][0]['value']['member_handle'] = 'BAD'
                failure = {}
                try:
                    worker.execute(bad, failure)
                except Exception as error:
                    assert not failure.get('drawing_started'), failure
                    rejected.append({'case': name, 'error': str(error)})
                else:
                    raise AssertionError('Invalid case was accepted: ' + name)
                assert read()['revision'] == before['revision']
            result = {}
            try:
                result.update(worker.execute(request, result))
            finally:
                (root / (year + '-edit.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
            assert result['status'] == 'EDITED_UNSAVED' and result['undo_group_closed']
            assert hashlib.sha256(output.read_bytes()).hexdigest() == original_hash
            members = list(iter_entities(doc.Blocks.Item('HOLE_C')))
            assert len(members) == 1 and members[0].Radius == 15 and members[0].Color == 3
            # 一次原生Undo必須還原全部成員與未選圖元，不以新計畫改回原值代替。
            if year == '2008':
                app.RunCommand('_U ')
            else:
                doc.SendCommand('_U\n')
            time.sleep(0.3)
            undone = read()
            (root / (year + '-undone.json')).write_text(json.dumps(undone, ensure_ascii=False, indent=2), encoding='utf-8')
            assert same(before['entities'], undone['entities'], worker), 'One Undo did not restore the entire snapshot'
            # 再次從新版本修改，再由測試明確保存自己的圖，核對重新開啟。
            request.update(document=undone['document'], revision=undone['revision'])
            repeat = {}
            repeat.update(worker.execute(request, repeat))
            doc.Save()
            doc.Close(False)
            doc = app.Documents.Open(str(output))
            reopened = read()
            for row in reopened['entities']:
                if row['kind'] == 'blockreference':
                    assert row['block']['members'][0]['properties']['Radius'] == 15
                    assert row['block']['members'][0]['properties']['Color'] == 3
            # 保留測試圖開啟供目視，原文件不關閉、不儲存。
            results.append({'year': year, 'status': 'PASS', 'rejected': rejected,
                            'undo': True, 'reopen': True, 'original_file_unchanged_before_save': True,
                            'path': str(output), 'previous_document': str(previous.Name)})
            (root / 'result.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
            print(year + ': PASS', flush=True)
    finally:
        pythoncom.CoUninitialize()


if __name__ == '__main__':
    main()
