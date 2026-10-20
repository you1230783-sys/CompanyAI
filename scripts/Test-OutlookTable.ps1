#requires -Version 5.1
<#
.SYNOPSIS
唯讀診斷 Classic Outlook 目前選定資料夾的 Table 陣列排列。
.DESCRIPTION
使用 Windows PowerShell 5.1 與內建 .NET，不需 Python、下載套件或編譯主程式。
只建立暫時的 Table 並讀取陣列尺寸，不更動信件、不輸出欄位內容或資料夾名稱。
直接檢查 IDispatch.Invoke 回傳的原生 SAFEARRAY，避免 .NET 自動轉換遮蔽維度差異。
.EXAMPLE
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\Test-OutlookTable.ps1
#>
[CmdletBinding()]
param([switch]$SelfTest)
$ErrorActionPreference = 'Stop'

# 這段橋接只取得原生回傳值，沒有讀取郵件正文／地址簿的介面。
if (-not ('OutlookTableProbe.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using EXCEPINFO = System.Runtime.InteropServices.ComTypes.EXCEPINFO;

namespace OutlookTableProbe {
    public static class Native {
        [StructLayout(LayoutKind.Sequential)]
        struct Parameters {
            public IntPtr Arguments, NamedArguments;
            public uint ArgumentCount, NamedCount;
        }
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        delegate int GetNames(IntPtr self, ref Guid iid, IntPtr names,
                             uint count, uint locale, out int id);
        [UnmanagedFunctionPointer(CallingConvention.StdCall)]
        delegate int Invoke(IntPtr self, int id, ref Guid iid, uint locale,
                            ushort flags, ref Parameters args, IntPtr result,
                            out EXCEPINFO exception, out uint badArgument);
        [DllImport("oleaut32.dll")]
        static extern int VariantClear(IntPtr value);
        [DllImport("oleaut32.dll")]
        static extern uint SafeArrayGetDim(IntPtr array);
        [DllImport("oleaut32.dll")]
        static extern int SafeArrayGetLBound(IntPtr array, uint dimension, out int bound);
        [DllImport("oleaut32.dll")]
        static extern int SafeArrayGetUBound(IntPtr array, uint dimension, out int bound);

        // VARIANT 在 x64 為24 bytes、x86為16 bytes，資料聯集皆從 offset 8 開始。
        static IntPtr NewVariant() {
            int size = IntPtr.Size == 8 ? 24 : 16;
            IntPtr value = Marshal.AllocCoTaskMem(size);
            Marshal.Copy(new byte[size], 0, value, size);
            return value;
        }

        public static string Read(object table, int requested, int expectedColumns) {
            IntPtr dispatch = Marshal.GetIDispatchForObject(table);
            IntPtr name = Marshal.StringToCoTaskMemUni("GetArray");
            IntPtr names = Marshal.AllocCoTaskMem(IntPtr.Size);
            IntPtr argument = NewVariant(), result = NewVariant();
            try {
                // IDispatch vtable: IUnknown三項、GetTypeInfo兩項、GetIDsOfNames、Invoke。
                IntPtr vtable = Marshal.ReadIntPtr(dispatch);
                var getNames = (GetNames)Marshal.GetDelegateForFunctionPointer(
                    Marshal.ReadIntPtr(vtable, 5 * IntPtr.Size), typeof(GetNames));
                var invoke = (Invoke)Marshal.GetDelegateForFunctionPointer(
                    Marshal.ReadIntPtr(vtable, 6 * IntPtr.Size), typeof(Invoke));
                Marshal.WriteIntPtr(names, name);
                Guid iid = Guid.Empty;
                int id;
                Marshal.ThrowExceptionForHR(getNames(dispatch, ref iid, names, 1, 0, out id));
                Marshal.WriteInt16(argument, 3); // VT_I4，與主程式的 MaxRows 型別一致。
                Marshal.WriteInt32(argument, 8, requested);
                var args = new Parameters { Arguments = argument, ArgumentCount = 1 };
                EXCEPINFO exception;
                uint badArgument;
                int hr = invoke(dispatch, id, ref iid, 0, 3, ref args, result,
                                out exception, out badArgument);
                // 不輸出 COM 錯誤內可能包含的郵件內容，只保留數值錯誤碼。
                if (hr < 0) return String.Format("GetArray({0}): HRESULT=0x{1:X8}; SCODE=0x{2:X8}",
                                                requested, hr, exception.scode);
                int vt = (ushort)Marshal.ReadInt16(result);
                string prefix = String.Format("GetArray({0}): VT=0x{1:X4}; ", requested, vt);
                if (vt == 0) return prefix + "EMPTY";
                if (vt != 0x200C) return prefix + "Unexpected type (expected ARRAY|VARIANT)";
                IntPtr array = Marshal.ReadIntPtr(result, 8);
                if (array == IntPtr.Zero) return prefix + "NULL SAFEARRAY";
                uint dimensions = SafeArrayGetDim(array);
                var parts = new List<string>();
                var lengths = new List<long>();
                for (uint dim = 1; dim <= dimensions; ++dim) {
                    int lower, upper;
                    Marshal.ThrowExceptionForHR(SafeArrayGetLBound(array, dim, out lower));
                    Marshal.ThrowExceptionForHR(SafeArrayGetUBound(array, dim, out upper));
                    long length = Math.Max(0, (long)upper - lower + 1);
                    lengths.Add(length);
                    parts.Add(String.Format("D{0}=[{1}..{2}] length={3}", dim, lower, upper, length));
                }
                string layout = "UNKNOWN";
                if (dimensions == 2) {
                    bool columnsFirst = lengths[0] == expectedColumns && lengths[1] <= requested;
                    bool rowsFirst = lengths[1] == expectedColumns && lengths[0] <= requested;
                    layout = columnsFirst && rowsFirst ? "AMBIGUOUS (square)" :
                             columnsFirst ? "D1=COLUMNS, D2=ROWS" :
                             rowsFirst ? "D1=ROWS, D2=COLUMNS" : "SIZE MISMATCH";
                }
                return prefix + "dimensions=" + dimensions + "; " +
                       String.Join("; ", parts) + "; " + layout;
            } finally {
                VariantClear(result);
                VariantClear(argument);
                Marshal.FreeCoTaskMem(result);
                Marshal.FreeCoTaskMem(argument);
                Marshal.FreeCoTaskMem(names);
                Marshal.FreeCoTaskMem(name);
                Marshal.Release(dispatch);
            }
        }
    }

    // 自檢只使用合成 COM 物件，不啟動或連接 Outlook。
    [ComVisible(true), ClassInterface(ClassInterfaceType.AutoDispatch)]
    public class Fixture {
        public bool Transpose;
        public object GetArray(int maxRows) {
            return Transpose ? new object[9, maxRows] : new object[maxRows, 9];
        }
    }
}
'@
}

if ($SelfTest) {
    $fixture = New-Object OutlookTableProbe.Fixture
    $results = foreach ($transpose in @($false, $true)) {
        $fixture.Transpose = $transpose
        [OutlookTableProbe.Native]::Read($fixture, 1, 9)
        [OutlookTableProbe.Native]::Read($fixture, 100, 9)
    }
    $results | ForEach-Object { Write-Output $_ }
    if (($results | Where-Object { $_ -match 'D1=COLUMNS, D2=ROWS' }).Count -ne 2 -or
        ($results | Where-Object { $_ -match 'D1=ROWS, D2=COLUMNS' }).Count -ne 2) {
        throw 'Native COM array self-test failed.'
    }
    Write-Output 'PASS: both native layouts; GetArray(1) and GetArray(100). Outlook was not accessed.'
    return
}

$report = New-Object 'System.Collections.Generic.List[string]'
function Write-ProbeLine([string]$Text) {
    $report.Add($Text)
    Write-Host $Text
}
$outlook = $null
$explorer = $null
$folder = $null
try {
    Write-ProbeLine ('Outlook Table probe v1 | ' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))
    Write-ProbeLine ('PowerShell=' + $PSVersionTable.PSVersion + '; ProcessBits=' + ([IntPtr]::Size * 8))
    $outlook = [Runtime.InteropServices.Marshal]::GetActiveObject('Outlook.Application')
    Write-ProbeLine ('OutlookVersion=' + $outlook.Version)
    $explorer = $outlook.ActiveExplorer()
    if ($null -eq $explorer) { throw 'Open Classic Outlook and select a non-empty mail folder first.' }
    $folder = $explorer.CurrentFolder
    $common = @('EntryID', 'MessageClass', 'Subject', 'SentOn', 'LastModificationTime')
    foreach ($visible in @($false, $true)) {
        $table = $null
        $columns = $null
        try {
            Write-ProbeLine ('--- Mode=' + $(if ($visible) { 'Visible headers' } else { 'Common headers' }) + ' ---')
            $table = $folder.GetTable('')
            $columns = $table.Columns
            $columns.RemoveAll()
            $names = @($common)
            if ($visible) { $names += @('ReceivedTime', 'SenderName', 'To', 'CC') }
            foreach ($name in $names) {
                $column = $columns.Add($name)
                [void][Runtime.InteropServices.Marshal]::ReleaseComObject($column)
                Write-ProbeLine ('Add ' + $name + '=OK')
            }
            if ($visible) {
                try {
                    $column = $columns.Add('ConversationID')
                    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($column)
                    $names += 'ConversationID'
                    Write-ProbeLine 'Add ConversationID=OK'
                } catch { Write-ProbeLine 'Add ConversationID=UNSUPPORTED (continuing)' }
            }
            $sortField = if ($visible) { 'ReceivedTime' } else { 'SentOn' }
            $table.Sort($sortField, $true)
            Write-ProbeLine ('Sort=' + $sortField + ' descending; ExpectedColumns=' + $names.Count + '; ActualColumns=' + $columns.Count)
            Write-ProbeLine ('TableRowCount=' + $table.GetRowCount())
            foreach ($requested in @(1, 100)) {
                Write-ProbeLine ([OutlookTableProbe.Native]::Read($table, $requested, $names.Count))
            }
        } catch {
            Write-ProbeLine ('MODE FAILED: ' + $_.Exception.GetType().Name + '; HRESULT=0x' + $_.Exception.HResult.ToString('X8'))
        } finally {
            foreach ($com in @($columns, $table)) {
                if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
                    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($com)
                }
            }
        }
    }
} catch {
    Write-ProbeLine ('PROBE FAILED: ' + $_.Exception.GetType().Name + '; HRESULT=0x' + $_.Exception.HResult.ToString('X8'))
    Write-ProbeLine 'Please open Classic Outlook, select a non-empty mail folder, and run as the same normal Windows user.'
} finally {
    foreach ($com in @($folder, $explorer, $outlook)) {
        if ($null -ne $com -and [Runtime.InteropServices.Marshal]::IsComObject($com)) {
            [void][Runtime.InteropServices.Marshal]::ReleaseComObject($com)
        }
    }
    $path = Join-Path $PSScriptRoot ('OutlookTableProbe-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.txt')
    try {
        $report | Set-Content -LiteralPath $path -Encoding UTF8
        Write-Host ('Report saved: ' + $path)
    } catch { Write-Host 'Could not save report. Please copy the output above.' }
}
