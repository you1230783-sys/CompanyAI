"""獨立重開本案DWG核對；只接受案例目錄內的成果，不代模型修圖。"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def persona_facts_delivered(conversation):
    """此runner只有初始要求與預存角色補充會新增user訊息；助理回覆不算補充。"""
    return sum(message.get('role') == 'user' for message in conversation.get('messages', [])) >= 2


def version_review(report, requested):
    """以原生報告核對建圖版本；錯版產物只在實際建立它的CAD唯讀檢查。

    不能用2008另開2022產物，冒充2008建圖成功，也避免觸發舊CAD的版本錯誤模態框。
    回傳實際ProgID與不符訊息；缺少版本證據直接停止，不推測。
    """
    selected = report.get('selected')
    actual = selected.get('progid') if isinstance(selected, dict) else None
    if not isinstance(actual, str) or not actual or actual.casefold() == 'latest':
        raise ValueError('原生成功報告缺少確切CAD ProgID，不能確認建圖版本')
    mismatch = (f'要求{requested}，實際在{actual}建立圖面；不能算指定版本建圖通過'
                if actual.casefold() != requested.casefold() else None)
    return actual, mismatch


def summarize_artifacts(results):
    """每個交付檔分別核對；任一錯版或資料問題都不能被另一份成功蓋過。"""
    if not results:
        raise ValueError('沒有已驗證的CAD產物')
    mismatch = any(result.get('version_mismatch') for result in results)
    status = ('WRONG_CAD_VERSION' if mismatch else 'DATA_ISSUES'
              if any(result['status'] != 'DATA_PASS_VISUAL_PENDING' for result in results)
              else 'DATA_PASS_VISUAL_PENDING')
    return dict(status=status, multiple_artifacts=True, artifact_count=len(results),
                artifacts=results, version_mismatch=mismatch,
                failures=[failure for result in results for failure in result.get('failures', [])],
                coverage='逐份獨立重開與原生資料核對；全部交付檔仍須實際畫面及版本用途覆核')


def rectangle_sizes(rows):
    """辨識獨立四邊直線環及原生封閉折線；不從文字標籤推測機台數量。"""
    sizes = []
    edges = []
    def point(p):
        return tuple(round(float(v), 3) for v in p[:3])
    def axis(a,b):
        return a[2] == b[2] and ((a[0] == b[0]) != (a[1] == b[1]))
    def inspect(points):
        if len(points) != 4:
            return
        xs, ys = sorted({p[0] for p in points}), sorted({p[1] for p in points})
        if len(xs) == len(ys) == 2 and len(set(points)) == 4:
            if set((p[0], p[1]) for p in points) == {(x,y) for x in xs for y in ys}:
                sizes.append(sorted((xs[1]-xs[0], ys[1]-ys[0])))
    for row in rows:
        p = row['properties']
        if row['kind'] == 'line' and 'StartPoint' in p and 'EndPoint' in p:
            edges.append((point(p['StartPoint']), point(p['EndPoint'])))
        elif row['kind'] == 'polyline' and row.get('geometry', {}).get('closed'):
            vertices = [point(p) for p in row['geometry'].get('vertices', [])]
            if vertices and vertices[0] == vertices[-1]:
                vertices.pop()
            if vertices and all(axis(a,b) for a,b in zip(vertices,vertices[1:]+vertices[:1])):
                inspect(vertices)
    links = defaultdict(list)
    for index, (a,b) in enumerate(edges):
        links[a].append(index)
        links[b].append(index)
    visited = set()
    for first in range(len(edges)):
        if first in visited:
            continue
        component, stack = set(), [first]
        while stack:
            index = stack.pop()
            if index in component:
                continue
            component.add(index)
            for p in edges[index]:
                stack.extend(i for i in links[p] if i not in component)
        visited.update(component)
        points = {p for i in component for p in edges[i]}
        if (len(component) == len(points) == 4 and all(len(links[p]) == 2 for p in points)
                and all(axis(*edges[i]) for i in component)):
            inspect(list(points))
    return sizes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--folder', type=Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--close', action='store_true')
    parser.add_argument('--report', type=Path, help='只核對本案某份原生成功報告；用於逐份覆核')
    parser.add_argument('--result-dir', type=Path, help='本案例內的獨立覆核輸出目錄')
    args = parser.parse_args()
    folder = args.folder.resolve(strict=True)
    sys.path.insert(0, str(args.runtime.resolve(strict=True)))
    import pythoncom
    import worker
    from cad_existing import iter_entities
    worker.BULK_ENABLED = True
    reports = [(p, json.loads(p.read_text(encoding='utf-8')))
               for p in sorted((folder/'project/_AI_Output').glob('*/cad-report.json'))]
    delivered = [(p,r) for p,r in reports if r.get('status') == 'DATA_VERIFIED_VISUAL_PENDING']
    if args.report:
        delivered = [(p,r) for p,r in delivered if p.resolve() == args.report.resolve(strict=True)]
    if not delivered:
        raise ValueError('沒有本案的原生成功報告；不把預定路徑當成果')
    destination = args.result_dir.resolve() if args.result_dir else folder
    if not destination.is_relative_to(folder):
        raise ValueError('覆核輸出必須留在本案例目錄')
    destination.mkdir(parents=True, exist_ok=True)
    if len(delivered) > 1:
        # 各檔使用獨立子程序串行讀取；不把多個COM執行緒同時送入同一CAD。
        # 子報告、快照與雜湊分開保存，不能只挑最後一份而略過前版。
        paths = [str((folder/'project'/report['intended_path']).resolve(strict=True)).casefold()
                 for _, report in delivered]
        if len(set(paths)) != len(paths):
            raise ValueError('多份成功報告指向相同DWG，不能重複計算產物')
        results = []
        for report_path, _ in delivered:
            artifact_folder = destination/'artifact-verifications'/report_path.parent.name
            command = [sys.executable, '-I', '-B', str(Path(__file__).resolve()),
                       '--runtime', str(args.runtime), '--folder', str(folder), '--version', args.version,
                       '--report', str(report_path), '--result-dir', str(artifact_folder)]
            if args.close:
                command.append('--close')
            subprocess.run(command, check=True, timeout=650,
                           creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            if not args.close:
                results.append(json.loads((artifact_folder/'independent-verification.json').read_text(encoding='utf-8')))
        if not args.close:
            summary = summarize_artifacts(results)
            (destination/'independent-verification.json').write_text(
                json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
            print(json.dumps(dict(status=summary['status'], artifact_count=len(results))))
        return
    report_path, report = delivered[0]
    inspection_version, version_mismatch = version_review(report, args.version)
    path = (folder/'project'/report['intended_path']).resolve(strict=True)
    if not path.is_relative_to(folder/'project/_AI_Output') or path.suffix.lower() != '.dwg':
        raise ValueError('成果不在本案例的專用資料夾')
    before = hashlib.sha256(path.read_bytes()).hexdigest()
    pythoncom.CoInitialize()
    try:
        worker.execute(dict(action='active_probe', version=inspection_version), {})
        app = worker.BULK_APP
        existing = [d for d in iter_entities(app.Documents)
                    if str(d.FullName).casefold() == str(path).casefold()]
        if args.close:
            if len(existing) != 1:
                raise ValueError('只能關閉本案已知且唯一的文件')
            existing[0].Close(False)
            assert before == hashlib.sha256(path.read_bytes()).hexdigest()
            print(json.dumps(dict(closed=str(path), file_unchanged=True)))
            return
        if existing:
            raise ValueError('獨立重開前檔案已開啟，保留現場，不猜測其狀態')
        doc = app.Documents.Open(str(path))
        state = worker.execute(dict(action='active_snapshot', version=inspection_version), {})
        assert str(doc.FullName).casefold() == str(path).casefold()
        rows = state['entities']
        failures = [version_mismatch] if version_mismatch else []
        if state['units_code'] != 4:
            failures.append('圖面單位不是毫米')
        if not rows or len({r['handle'] for r in rows}) != len(rows):
            failures.append('圖元為空或Handle不唯一')
        counts = Counter(r['kind'] for r in rows)
        if counts['dimension'] < 2:
            failures.append('缺少至少兩個原生主要尺寸，需人工確認交付完整性')
        texts = [r['properties']['TextString'] for r in rows if 'TextString' in r['properties']]
        sizes = rectangle_sizes(rows)
        measurements = [r['properties']['Measurement'] for r in rows
                        if r['kind']=='dimension' and isinstance(r['properties'].get('Measurement'), (int,float))]
        conversation = json.loads((folder/'result.json').read_text(encoding='utf-8'))
        # 只有角色已實際提供數字條件，才以該條件驗收；不將隱藏的預存答案當模型已知。
        if persona_facts_delivered(conversation):
            expected_dimensions = {'two_floor_house':[8000,12000],
                                   'small_factory':[40000,25000],
                                   'large_factory':[100000,60000]}.get(conversation['case'], [])
            for expected in expected_dimensions:
                if not any(abs(m-expected) < 1 for m in measurements):
                    failures.append(f'未找到{expected}毫米的原生整體尺寸，需覆核單位換算及主要尺寸')
            wanted = {'small_factory': [(1500,2000,12)],
                      'large_factory': [(1500,2000,20),(2000,3000,20),(2500,4000,20)]}.get(conversation['case'], [])
            for width,height,minimum in wanted:
                found = sum(abs(s[0]-width)<1 and abs(s[1]-height)<1 for s in sizes)
                if found < minimum:
                    failures.append(f'獨立矩形辨識只找到{found}台{width}×{height}設備，要求{minimum}台；如有共邊須人工覆核')
        if hashlib.sha256(path.read_bytes()).hexdigest() != before:
            failures.append('獨立讀回後DWG磁碟內容改變')
        (destination/'independent-snapshot.json').write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding='utf-8')
        result = dict(status='WRONG_CAD_VERSION' if version_mismatch else 'DATA_ISSUES' if failures else 'DATA_PASS_VISUAL_PENDING',
            dwg=str(path), sha256=before, units=state['units_code'], entities=len(rows), kinds=counts,
            requested_version=args.version, actual_version=inspection_version,
            version_mismatch=bool(version_mismatch),
            texts=texts, rectangle_sizes=sizes, failures=failures, original_report=str(report_path),
            dimension_measurements=measurements,
            coverage='獨立讀回、毫米單位、原生主要尺寸、已提供的住宅廠房尺度與可辨識機台數；用途、家具、通行、安裝及外觀另核對')
        (destination/'independent-verification.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
        # 只調整本次已重開的測試文件視野，供後續真正CAD畫面核對；不儲存。
        try:
            if inspection_version == 'Icad.Application.1':
                doc.ActiveViewport.ZoomExtents()
                drawing = object.__new__(worker.Drawing)
                drawing.app, drawing.doc, drawing.legacy = app, doc, True
                result['view_check'] = {}
                drawing.verify_legacy_zoom('ZOOM EXTENTS', result['view_check'])
            else:
                app.ZoomExtents()
        except Exception:
            result['view_fit_unavailable'] = True
        (destination/'independent-verification.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps({k:result[k] for k in ('status','dwg','entities','kinds','failures')}, ensure_ascii=True))
    finally:
        pythoncom.CoUninitialize()


if __name__ == '__main__':
    main()
