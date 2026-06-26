# hwp-convert-cli

`hwp-convert-cli` is a Windows CLI for converting Hancom document files by automating an installed Hancom Office Hangul instance. It also builds a short `hwpc` executable for day-to-day use.

The implementation follows the core approach used by `ssj1977/hwp2pdf`: create a Hancom automation object, open the source file, and call `SaveAs(output, targetType, "")`. Version 1 intentionally supports PDF through `SaveAs("PDF")` only; virtual PDF printer conversion is not included yet.

## Requirements

- Windows
- Hancom Office Hangul with COM automation support
- Rust toolchain for building
- Optional but recommended: `FilePathCheckerModuleExample.dll` from Hancom automation samples, to allow unattended local-file access

## Build

```powershell
cargo build --release
```

The executable is created at:

```text
target\release\hwp-convert-cli.exe
target\release\hwpc.exe
```

## Usage

```powershell
hwp-convert-cli convert .\sample.hwp --to hwpx
hwp-convert-cli convert .\sample.hwp --to pdf --output .\sample.pdf --overwrite
hwp-convert-cli convert .\docs --to pdf --out-dir .\converted --recursive --json
```

The same commands can be called with the shorter alias:

```powershell
hwpc convert .\sample.hwp --to hwpx
hwpc convert .\sample.hwp --to pdf --output .\sample.pdf --overwrite
hwpc convert .\docs --to pdf --out-dir .\converted --recursive --json
```

Supported targets:

```text
pdf, hwpx, hwp, hml, html, odt, docx, txt, rtf
```

For unattended conversion, pass the FilePathChecker DLL:

```powershell
hwp-convert-cli convert .\sample.hwp --to pdf --file-path-checker-dll C:\path\to\FilePathCheckerModuleExample.dll
```

or set:

```powershell
$env:HWP_FILE_PATH_CHECKER_DLL = 'C:\path\to\FilePathCheckerModuleExample.dll'
```

## Notes From `ssj1977/hwp2pdf`

The reference implementation is a C# WinForms app. Its conversion pipeline is:

1. Check that Hancom Office registry keys exist.
2. Create `HwpObjectLib.HwpObject`.
3. Register `FilePathCheckerModuleExample.dll` under `HKCU\Software\HNC\HwpAutomation\Modules`.
4. Call `RegisterModule("FilePathCheckDLL", "FilePathCheckerModuleExample")`.
5. Open each document with `Open(path, "", "lock:false;forceopen:true;suspendpassword:true;")`.
6. Convert most formats with `SaveAs(save_path, target_type, "")`.
7. Optionally convert PDF through a virtual printer. This project does not implement that v1 path.

The output type names mirrored by this CLI are Hancom `SaveAs` target names, for example `PDF`, `HWPX`, `OOXML`, and `UNICODE`.
