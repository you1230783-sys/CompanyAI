"""核對0.9.4搬移的函式本體；只允許空白與已列明的模組路徑調整。

不需要第三方Python套件。字串／字元／註解保留原文，避免忽略字串內空白變更。
這是搬移輔助檢查，不能取代編譯、可見性審查及實際回歸。
"""
from pathlib import Path
import hashlib
from functools import lru_cache
import json
import re
import subprocess
import sys


def tokens(source):
    """回傳非空白Rust詞元及位置；辨識巢狀註解、raw string與生命週期。"""
    index = 0
    while index < len(source):
        start = index
        if source[index].isspace():
            index += 1
            continue
        if source.startswith('//', index):
            end = source.find('\n', index)
            index = len(source) if end < 0 else end
        elif source.startswith('/*', index):
            depth = 1
            index += 2
            while depth:
                if source.startswith('/*', index):
                    depth += 1
                    index += 2
                elif source.startswith('*/', index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
                if index > len(source):
                    raise ValueError('未結束的區塊註解')
        elif match := re.match(r'(?:br|cr|r)(#*)"', source[index:]):
            closing = '"' + match[1]
            end = source.find(closing, index + len(match[0]))
            if end < 0:
                raise ValueError('未結束的raw string')
            index = end + len(closing)
        elif source[index] == '"':
            index += 1
            while index < len(source):
                if source[index] == '\\':
                    index += 2
                elif source[index] == '"':
                    index += 1
                    break
                else:
                    index += 1
        elif match := re.match(r"'(?:\\u\{[0-9a-fA-F]+\}|\\.|[^'\\\n])'", source[index:]):
            index += len(match[0])
        elif match := re.match(r'[A-Za-z_][A-Za-z_0-9]*|[0-9]+', source[index:]):
            index += len(match[0])
        else:
            index += 1
        yield source[start:index], start, index


@lru_cache(maxsize=32)
def functions(source):
    """本次搬移僅包含有實體函式本體的項目，不處理macro產生的函式。"""
    result = []
    pattern = r'(?m)^(?P<indent> *)(?:(?:pub(?:\([^\n]*?\))?|unsafe|extern "system") +)*fn (?P<name>\w+)'
    for match in re.finditer(pattern, source):
        depth = 0
        body = None
        for value, start, end in tokens(source[match.end():]):
            start += match.end()
            end += match.end()
            if value == '{':
                if body is None:
                    body = start
                depth += 1
            elif value == '}':
                depth -= 1
                if body is not None and depth == 0:
                    result.append(dict(name=match['name'], line=source.count('\n', 0, match.start())+1,
                                       start=match.start(), body=body, end=end, indent=len(match['indent'])))
                    break
    return result


def digest(body):
    return hashlib.sha256(json.dumps([t for t, _, _ in tokens(body)],ensure_ascii=False).encode('utf-8')).hexdigest()


def verify(manifest):
    data = json.loads(Path(manifest).read_text(encoding='utf-8'))
    sources = {}
    checked = 0
    for entry in data['functions']:
        key = (data['source_commit'], entry['from'])
        if key not in sources:
            sources[key] = subprocess.check_output(['git', 'show', key[0]+':'+key[1]]).decode('utf-8')
        old = sources[key]
        original = next(f for f in functions(old) if f['name']==entry['name'] and f['line']==entry['line'])
        old_body = old[original['body']:original['end']]
        assert digest(old_body) == entry['body_sha256'], entry
        expected = old_body
        for before, after in entry['path_rewrites']:
            expected = expected.replace(before, after)
        new = Path(entry['to']).read_text(encoding='utf-8')
        candidates = [f for f in functions(new) if f['name']==entry['name']]
        assert len(candidates)==1, (entry, '目標名稱重複或遺失')
        target = candidates[0]
        assert digest(expected)==digest(new[target['body']:target['end']]), (entry, '函式本體不符')
        checked += 1
    print(json.dumps({'result':'PASS','manifest':str(manifest),'functions':checked},ensure_ascii=False))


if __name__ == '__main__':
    for manifest in sys.argv[1:]:
        verify(manifest)
