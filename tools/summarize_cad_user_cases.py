"""整理批次模型往返與案例判定；原始回覆保留在私有證據目錄。"""
from collections import Counter
import json
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve(strict=True)
cases = {}
for event_file in sorted((root/'cases').glob('*/events.jsonl')):
    folder = event_file.parent
    ids = set()
    for line in event_file.read_text(encoding='utf-8').splitlines():
        ids.add(json.loads(line)['run_id'])
    cases[folder.name] = dict(run_ids=sorted(ids), model_turns=0, model_seconds=0,
        tool_counts=Counter(), max_input_tokens=0, output_tokens=0, image_inputs=False,
        image_tools=False, generated_operations=[], staged_parts=[], plan_commits=0,
        output_limit_rounds=0, last_message=None)
    if (folder/'case-result.json').exists():
        cases[folder.name]['assessment'] = json.loads((folder/'case-result.json').read_text(encoding='utf-8'))
for path in sorted((root/'bridge').glob('turn-*.response.json')):
    response=json.loads(path.read_text(encoding='utf-8'))
    request=json.loads(path.with_name(path.name.replace('.response.json','.request.json')).read_text(encoding='utf-8'))
    case=next((c for c in cases.values() if request['context']['run_id'] in c['run_ids']),None)
    if case is None:
        continue
    case['model_turns']+=1
    case['output_limit_rounds'] += response['native_completion']['choices'][0].get('finish_reason') == 'length'
    case['model_seconds']+=response['elapsed_seconds']
    usage=response['native_completion'].get('usage',{})
    case['output_tokens']+=usage.get('completion_tokens',0)
    case['max_input_tokens']=max(case['max_input_tokens'],usage.get('prompt_tokens',0))
    case['image_tools'] |= any(t['function']['name']=='analyze_image' for t in request.get('tools',[]))
    for message in request['messages']:
        content=message.get('content')
        if isinstance(content,list):
            case['image_inputs'] |= any(isinstance(p,dict) and p.get('type')=='image_url' for p in content)
    for call in response['native_completion']['choices'][0]['message'].get('tool_calls') or []:
        name=call['function']['name']
        case['tool_counts'][name]+=1
        args=json.loads(call['function']['arguments'])
        if name in ('cad_draw','cad_active_edit','cad_edit'):
            case['generated_operations'].append(dict(tool=name,operations=len(args.get('operations',[])),edits=len(args.get('edits',[]))))
        if name == 'cad_plan_part':
            case['staged_parts'].append(dict(part_id=args.get('part_id'),plan_revision=args.get('plan_revision'),operations=len(args.get('operations',[]))))
        if name == 'cad_plan_commit':
            case['plan_commits'] += 1
        if name in ('finish','ask_user'):
            case['last_message']=args.get('message')
for case in cases.values():
    case['model_seconds']=round(case['model_seconds'],3)
    if case['staged_parts']:
        # 只判定模型實際提交大小，不將暫存段數誤當幾何或設計已完成。
        case['staged_limits_observed'] = all(1 <= part['operations'] <= 20 for part in case['staged_parts'])
(root/'model-summary.json').write_text(json.dumps(cases,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({name:{k:c[k] for k in ('model_turns','model_seconds','tool_counts')} for name,c in cases.items()},ensure_ascii=True))
