"""固定的副本測試驅動器。來源由原生層綁定版本，測試不能替換成模型重寫的函式。

測試程式定義 unittest.TestCase，透過 load_target() 載入完整來源；可先以
unittest.mock.patch.dict(sys.modules, ...) 模擬外部套件。所有檔案僅在本次
AppContainer 的私有工作目錄，沒有專案路徑、登入資料或網路能力。
"""
import hashlib
import os
import sys
import tempfile
import traceback
import types
import unittest
from unittest import mock

source = texts["source"]
request = metadata["source"]
workspace = request["workspace"]
os.chdir(workspace)
tempfile.tempdir = workspace
os.environ["TEMP"] = os.environ["TMP"] = workspace
reports = []


class Results(unittest.TestResult):
    def __init__(self):
        super().__init__()
        self.missing = 0

    def addError(self, test, error):
        if issubclass(error[0], (ModuleNotFoundError, ImportError)):
            self.missing += 1
        super().addError(test, error)


for specification in request["tests"]:
    loaded = [0]
    # Windows Python 3.13 的 mkdtemp(0700) 會重建僅使用者／管理員可用的
    # ACL，使受限 AppContainer 連自己建的目錄也無法進入。此目錄的父層
    # 已是本次私有容器；使用一般 mkdir 繼承原 ACL，不改動共用 TEMP 權限。
    test_dir = os.path.join(workspace, "case_" + str(len(reports)))
    os.mkdir(test_dir)
    os.chdir(test_dir)

    def load_target(argv=None, as_main=False):
        """執行綁定的完整來源，不做 AST 抽取；頂層程式同樣受 OS 隔離。"""
        module = types.ModuleType("__main__" if as_main else "tested_target")
        module.__file__ = os.path.join(test_dir, "target.py")
        previous_argv = sys.argv
        sys.argv = [module.__file__] + list(argv or [])
        loaded[0] += 1
        try:
            with mock.patch.dict(sys.modules, {module.__name__: module}):
                exec(compile(source, module.__file__, "exec", dont_inherit=True), module.__dict__)
        finally:
            sys.argv = previous_argv
        return module

    report = {"id": specification["id"], "requirement_ids": specification["requirement_ids"],
              "mocked_dependencies": specification["mocked_dependencies"], "status": "unavailable",
              "tests_run": 0, "target_loads": 0, "failures": [], "skipped": 0}
    try:
        scope = {"__name__": "test_case", "unittest": unittest, "mock": mock,
                 "load_target": load_target, "workspace": test_dir}
        exec(compile(specification["code"], "<acceptance-test>", "exec"), scope)
        suite = unittest.TestSuite()
        for value in list(scope.values()):
            if isinstance(value, type) and issubclass(value, unittest.TestCase) and value is not unittest.TestCase:
                suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(value))
        outcome = Results()
        suite.run(outcome)
        report["tests_run"] = outcome.testsRun
        report["skipped"] = len(outcome.skipped)
        report["failures"] = [detail[-1500:] for _, detail in (outcome.failures + outcome.errors)[:3]]
        if outcome.unexpectedSuccesses:
            report["status"] = "failed"
            report["failures"] = ["存在意外成功的expectedFailure案例，請核對測試設計。"]
        elif outcome.failures or outcome.errors:
            report["status"] = "unavailable" if not outcome.failures and outcome.missing == len(outcome.errors) else "failed"
        elif outcome.testsRun and loaded[0] and not outcome.skipped and not outcome.expectedFailures:
            report["status"] = "passed"
        else:
            report["failures"] = ["沒有實際測試、未載入交付副本或存在跳過項目，不能標示通過。"]
    except BaseException:
        report["failures"] = [traceback.format_exc(limit=5)[-1500:]]
    report["target_loads"] = loaded[0]
    reports.append(report)

result = {"functional_tests_run": True, "source_sha256": hashlib.sha256(source.encode("utf-8")).hexdigest(),
          "python_version": sys.version.split()[0], "tests": reports,
          "notice": "測試只涵蓋所列案例；模擬依賴不代表真實服務已驗證。檔案位於隔離暫存區，不發布為專案成果。"}
