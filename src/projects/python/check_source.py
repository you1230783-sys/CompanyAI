"""固定語法檢查器，由Rust放入既有AppContainer執行；受檢原文只作資料。"""
import ast
import sys
import tokenize
import warnings

source = bytes.fromhex(metadata["source"]["encoded_hex"])
result = {"syntax_valid": False, "source_executed": False, "functional_tests_run": False,
          "python_version": sys.version.split()[0], "errors": [], "warnings": [], "outline": []}
try:
    encoding, _ = tokenize.detect_encoding(io.BytesIO(source).readline)
    decoded = source.decode(encoding)
    # 核對即將發布的bytes與編輯器文字相同，避免coding宣告與實際編碼不一致。
    if decoded != texts["source"]:
        raise ValueError("編碼宣告解出的文字與工作副本不符；請核對檔案編碼與coding宣告。")
    result["encoding"] = encoding
    with warnings.catch_warnings(record=True) as captured:
        warnings.simplefilter("always")
        tree = ast.parse(source, filename="<project-source>", mode="exec")
        # AST成功仍可能有return/nonlocal等範圍錯誤；編譯但不exec，也不寫pyc。
        compile(source, "<project-source>", "exec", dont_inherit=True)
    result["syntax_valid"] = True
    result["warnings"] = [{"message": str(w.message)[:250], "line": w.lineno}
                          for w in captured[:5]]
    symbols = [node for node in ast.walk(tree)
               if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef))]
    result["symbol_count"] = len(symbols)
    result["outline"] = [{"kind": type(node).__name__, "name": node.name[:120],
                          "line": node.lineno, "end_line": node.end_lineno}
                         for node in symbols[:20]]
    result["outline_truncated"] = len(symbols) > 20
except (SyntaxError, ValueError, UnicodeError, LookupError, RecursionError) as error:
    result["errors"] = [{"type": type(error).__name__, "message": str(error)[:500],
                         "line": getattr(error, "lineno", None),
                         "column": getattr(error, "offset", None),
                         "end_line": getattr(error, "end_lineno", None)}]
