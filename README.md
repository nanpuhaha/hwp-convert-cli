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

## Defaults

`hwpc` reads defaults in this order:

1. Built-in defaults
2. `~\.config\hwpc\config.json`
3. Environment variables
4. Explicit CLI flags

Example config:

```json
{
  "overwrite": true,
  "json": true,
  "recursive": false,
  "skip_existing": false,
  "file_path_checker_dll": "C:\\path\\to\\FilePathCheckerModuleExample.dll"
}
```

Use `HWPC_CONFIG` to point at a different config file:

```powershell
$env:HWPC_CONFIG = 'C:\Users\me\.config\hwpc\config.json'
```

Supported environment variables:

```text
HWPC_OVERWRITE=true|false
HWPC_SKIP_EXISTING=true|false
HWPC_JSON=true|false
HWPC_RECURSIVE=true|false
HWPC_FILE_PATH_CHECKER_DLL=C:\path\to\FilePathCheckerModuleExample.dll
```

CLI flags always win over defaults. For example, if config sets `"overwrite": true`, use `--no-overwrite` to disable it for one command.

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

### Installing FilePathCheckerModuleExample.dll

`FilePathCheckerModuleExample.dll` is not bundled with this crate or repository. It is a Hancom automation security module, so redistribution rights should be checked with Hancom before sharing the DLL publicly.

If you already have the DLL, `hwpc` can copy it into its config directory and update `config.json` for you:

```powershell
hwpc file-checker install C:\path\to\FilePathCheckerModuleExample.dll
```

This copies the DLL to:

```text
%USERPROFILE%\.config\hwpc\FilePathCheckerModuleExample.dll
```

and writes:

```json
{
  "file_path_checker_dll": "C:\\Users\\you\\.config\\hwpc\\FilePathCheckerModuleExample.dll"
}
```

Use `--force` to replace an existing installed copy:

```powershell
hwpc file-checker install C:\path\to\FilePathCheckerModuleExample.dll --force
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
