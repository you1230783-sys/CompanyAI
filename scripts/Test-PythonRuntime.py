"""裁剪後的真實科學套件驗收；與離線建置腳本一同封裝，不使用網路或外部資料。"""
import io
import importlib.machinery


def reject_runtime_compilation(*_args, **_kwargs):
    """本測試要求套件讀取既有 pyc；漏打包時不能靜默回退到編譯原始碼。"""
    raise AssertionError("Runtime package was compiled from source instead of verified bytecode")


importlib.machinery.SourceFileLoader.source_to_code = reject_runtime_compilation
import numpy as np
import pandas as pd
from scipy import optimize, signal, stats, interpolate
import matplotlib

matplotlib.use("Agg")
from matplotlib import pyplot as plt
import openpyxl

assert pd.DataFrame({"x": [1, 2, 3]}).x.sum() == 6
np.testing.assert_allclose(optimize.nnls(np.eye(2), np.array([2.0, 3.0]))[0], [2, 3])
assert signal.find_peaks([0, 1, 0, 2, 0])[0].tolist() == [1, 3]
assert abs(stats.norm.cdf(0) - 0.5) < 1e-12
assert float(interpolate.interp1d([0, 1], [2, 4])(0.5)) == 3
figure, axes = plt.subplots()
axes.plot([1, 2], [3, 4])
image = io.BytesIO()
figure.savefig(image, format="png")
plt.close(figure)
assert image.getvalue().startswith(b"\x89PNG\r\n\x1a\n")
book = openpyxl.Workbook()
book.active.append(["value", 3])
output = io.BytesIO()
book.save(output)
output.seek(0)
restored = openpyxl.load_workbook(output)
assert restored.active["B1"].value == 3
restored.close()
print("PASS pandas/numpy/nnls/find_peaks/stats/interpolate/Agg/openpyxl")
