"""串行執行一般使用者CAD案例；原始模型、原生報告、資料及視覺核對分別保存。"""
import argparse
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')


def validate_resume_manifest(saved, expected):
    """僅續跑同一套執行檔、runtime、案例及版本；不借用舊候選的結果。"""
    for key, value in expected.items():
        if saved.get(key) != value:
            raise ValueError('續跑設定不符或缺少驗證欄位：' + key)


def has_partial_failure(reports):
    """另一份成功產物不能蓋過尚待檢查的部分寫入或存檔失敗。"""
    return any(report.get('status') != 'DATA_VERIFIED_VISUAL_PENDING' and
               (report.get('drawing_started') or report.get('file_exists')) for report in reports)


def validate_completed_case(folder, item, visual_required):
    """已完成的案例必須仍有覆核證據與相同DWG，才能避免重跑。"""
    saved = json.loads((folder/'case-result.json').read_text(encoding='utf-8'))
    if saved != item:
        raise ValueError('案例與批次摘要不一致：' + folder.name)
    reports = [json.loads(path.read_text(encoding='utf-8'))
               for path in (folder/'project/_AI_Output').glob('*/cad-report.json')]
    if any(report.get('status') == 'DATA_VERIFIED_VISUAL_PENDING' for report in reports):
        data = json.loads((folder/'independent-verification.json').read_text(encoding='utf-8'))
        if visual_required and not (folder/'visual-review.json').is_file():
            raise ValueError('案例尚未完成實際畫面覆核：' + folder.name)
        artifacts = data.get('artifacts', [data])
        if len(artifacts) != sum(report.get('status') == 'DATA_VERIFIED_VISUAL_PENDING' for report in reports):
            raise ValueError('案例並非每份交付檔都已核對：' + folder.name)
        for artifact in artifacts:
            path = Path(artifact['dwg']).resolve(strict=True)
            if not path.is_relative_to(folder/'project/_AI_Output'):
                raise ValueError('覆核DWG不在本案輸出目錄')
            if hashlib.sha256(path.read_bytes()).hexdigest() != artifact['sha256']:
                raise ValueError('已覆核DWG後來變更：' + str(path))
    if has_partial_failure(reports):
        if visual_required and not (folder/'failure-review.json').is_file():
            raise ValueError('部分失敗圖面尚未人工檢查：' + folder.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executable', type=Path, required=True)
    parser.add_argument('--connection', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--case', action='append')
    parser.add_argument('--version', action='append')
    parser.add_argument('--pause-for-visual', action='store_true')
    parser.add_argument('--resume', action='store_true', help='僅從已覆核的案例邊界續跑相同候選，不覆寫既有案例')
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists() and not args.resume:
        raise ValueError('批次目錄已存在，不能覆寫舊實驗')
    cases_path = ROOT/'tools/cad_user_cases.json'
    cases = json.loads(cases_path.read_text(encoding='utf-8'))
    if args.case:
        assert set(args.case) <= {c['id'] for c in cases}
        assert len(set(args.case)) == len(args.case), '同批案例不得重複，重測請建立新批次'
        by_id = {c['id']: c for c in cases}
        cases = [by_id[case_id] for case_id in args.case]
    versions = args.version or ['Icad.Application.1','GStarCAD.Application.22']
    configuration = dict(cases=cases, versions=versions, executable=str(args.executable.resolve()),
                         executable_sha256=hashlib.sha256(args.executable.read_bytes()).hexdigest(),
                         runtime_sha256=hashlib.sha256((args.runtime/'manifest.json').read_bytes()).hexdigest(),
                         formal_runner=True, gui_submission=False)
    if args.resume:
        progress = json.loads((output/'progress.json').read_text(encoding='utf-8'))
        if progress.get('stage') not in ('stopped_at_case_boundary', 'complete'):
            raise ValueError('只能從已知案例邊界續跑，不能重送結果未知的任務')
        validate_resume_manifest(json.loads((output/'manifest.json').read_text(encoding='utf-8')), configuration)
        summary = json.loads((output/'summary.json').read_text(encoding='utf-8'))
        planned = [(case['id'], '2008' if version == 'Icad.Application.1' else '2022')
                   for case in cases for version in versions]
        if [(item['case'], item['year']) for item in summary] != planned[:len(summary)]:
            raise ValueError('已完成案例不是本批次的連續前段')
        for item in summary:
            validate_completed_case(output/(item['case']+'-'+item['year']), item, args.pause_for_visual)
        write(output/('resume-'+datetime.now().strftime('%Y%m%d_%H%M%S')+'.json'),
              dict(configuration=configuration, completed_cases=len(summary)))
    else:
        output.mkdir(parents=True)
        write(output/'manifest.json', dict(started=datetime.now().isoformat(), **configuration))
        summary = []
    (output/'controller.pid').write_text(str(os.getpid()), encoding='ascii')
    completed = {(item['case'], item['year']) for item in summary}
    for case in cases:
        for version in versions:
            # 在案例邊界可停下來診斷／更換候選版，不中斷未知的CAD寫入。
            if (output/'stop-after-case').exists():
                write(output/'progress.json', dict(stage='stopped_at_case_boundary', completed=len(summary)))
                return
            year = '2008' if version == 'Icad.Application.1' else '2022'
            if (case['id'], year) in completed:
                continue
            folder = output/(case['id']+'-'+year)
            if folder.exists():
                raise ValueError('已有未完成案例，不能覆寫或自動重跑：' + str(folder))
            progress = dict(case=case['id'], version=version, folder=str(folder), stage='model_and_cad')
            write(output/'progress.json', progress)
            print(json.dumps(progress), flush=True)
            log = output/(folder.name+'.log')
            with log.open('w', encoding='utf-8') as stream:
                process = subprocess.Popen([str(args.executable), str(args.connection), str(folder),
                    str(cases_path), case['id'], version], stdout=stream, stderr=subprocess.STDOUT,
                    creationflags=subprocess.CREATE_NO_WINDOW)
                (output/(folder.name+'.pid')).write_text(str(process.pid), encoding='ascii')
                try:
                    code = process.wait(timeout=2400)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                    write(output/'progress.json', dict(**progress, stopped=True, reason='本案逾時，CAD結果未知，整批停止'))
                    raise
            reports = [json.loads(p.read_text(encoding='utf-8'))
                       for p in sorted((folder/'project/_AI_Output').glob('*/cad-report.json'))]
            partial_failure = has_partial_failure(reports)
            item = dict(case=case['id'], year=year, process_exit=code,
                        reports=[dict(status=r.get('status'), error=r.get('error'), saved=r.get('saved')) for r in reports])
            result_path = folder/'result.json'
            result = json.loads(result_path.read_text(encoding='utf-8')) if result_path.is_file() else {}
            item['task_paused'] = bool(result.get('paused'))
            item['pause_reason'] = result.get('pause_reason')
            # 即使模型後來暫停或失敗，已產生的交付檔仍要核對，不因runner狀態略過。
            if any(r.get('status') == 'DATA_VERIFIED_VISUAL_PENDING' for r in reports):
                command = [str(args.runtime/'runtime/python.exe'), '-I', '-B',
                    str(ROOT/'tools/verify_cad_user_case.py'), '--runtime', str(args.runtime),
                    '--folder', str(folder), '--version', version]
                with (folder/'verification.log').open('w', encoding='utf-8') as stream:
                    verified = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT,
                        timeout=650, creationflags=subprocess.CREATE_NO_WINDOW)
                if verified.returncode:
                    item['status'] = 'VERIFICATION_FAILED'
                else:
                    item['data'] = json.loads((folder/'independent-verification.json').read_text(encoding='utf-8'))
                    item['status'] = item['data']['status']
                    if args.pause_for_visual:
                        write(output/'progress.json', {**progress, 'stage':'await_visual', 'review':case['review']})
                        print(json.dumps(dict(case=case['id'], year=year, stage='await_visual', folder=str(folder))), flush=True)
                        while not (folder/'visual-review.json').exists():
                            time.sleep(2)
                        item['visual'] = json.loads((folder/'visual-review.json').read_text(encoding='utf-8'))
                        if item['data']['status']=='DATA_PASS_VISUAL_PENDING' and item['visual'].get('status')=='PASS':
                            if code != 0 or item['task_paused'] or partial_failure:
                                item['status'] = 'DRAWING_PASS_TASK_INCOMPLETE'
                            else:
                                item['status'] = 'PASS'
                                subprocess.run(command+['--close'], check=True, timeout=60,
                                    creationflags=subprocess.CREATE_NO_WINDOW, stdout=subprocess.DEVNULL)
                        elif item['visual'].get('status') == 'ISSUES':
                            item['status'] = 'VISUAL_ISSUES'
                    # 畫面可讀或另一版能重開，都不能補足指定CAD版本的建圖驗證。
                    if item['data'].get('version_mismatch'):
                        item['status'] = 'WRONG_CAD_VERSION'
            elif reports:
                item['status'] = 'CAD_FAILED'
                if args.pause_for_visual and any(r.get('drawing_started') or r.get('file_exists') for r in reports):
                    write(output/'progress.json', {**progress, 'stage':'await_failure_review'})
                    print(json.dumps(dict(case=case['id'], year=year, stage='await_failure_review', folder=str(folder))), flush=True)
                    while not (folder/'failure-review.json').exists():
                        time.sleep(2)
                    item['failure_review'] = json.loads((folder/'failure-review.json').read_text(encoding='utf-8'))
            elif code != 0:
                item['status'] = 'RUNNER_FAILED'
            else:
                if result.get('paused'):
                    reason = result.get('pause_reason') or ''
                    item['status'] = 'PAUSED_OUTPUT_LIMIT' if '模型用完輸出額度' in reason else 'PAUSED'
                else:
                    item['status'] = 'NO_DRAWING_REVIEW_REPLY'
            if partial_failure and any(report.get('status') == 'DATA_VERIFIED_VISUAL_PENDING' for report in reports):
                item['status'] = 'CAD_FAILED_AFTER_DELIVERY'
                if args.pause_for_visual:
                    write(output/'progress.json', {**progress, 'stage':'await_failure_review'})
                    while not (folder/'failure-review.json').exists():
                        time.sleep(2)
                    item['failure_review'] = json.loads((folder/'failure-review.json').read_text(encoding='utf-8'))
            write(folder/'case-result.json', item)
            summary.append(item)
            write(output/'summary.json', summary)
            print(json.dumps(dict(case=case['id'], year=year, status=item['status'])), flush=True)
    write(output/'progress.json', dict(stage='complete', cases=len(summary)))


if __name__ == '__main__':
    main()
