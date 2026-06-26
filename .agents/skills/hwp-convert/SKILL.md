---
name: hwp-convert
description: Convert HWP/HWPX and related Hancom-readable documents to PDF, HWPX, DOCX, TXT, and other formats using the local hwp-convert-cli Rust CLI.
---

# HWP Convert

Use this skill when the user asks to convert `.hwp`, `.hwpx`, `.hml`, `.docx`, `.doc`, `.odt`, `.txt`, `.rtf`, or HTML documents through Hancom Office.

## Requirements

- Run on Windows.
- Hancom Office Hangul must be installed.
- Prefer the short repo CLI at `target\release\hwpc.exe`; if it does not exist, build it with `cargo build --release`.
- For unattended local-file access, use `FilePathCheckerModuleExample.dll` when available:
  - pass `--file-path-checker-dll <PATH>`, or
  - set `HWP_FILE_PATH_CHECKER_DLL`.
  - if the user already has the DLL, install it with `target\release\hwpc.exe file-checker install <PATH>`.

## Conversion Commands

Single file to HWPX:

```powershell
target\release\hwpc.exe convert .\input.hwp --to hwpx --overwrite --json
```

Single file to PDF:

```powershell
target\release\hwpc.exe convert .\input.hwp --to pdf --overwrite --json
```

Directory batch conversion:

```powershell
target\release\hwpc.exe convert .\docs --to pdf --out-dir .\converted --recursive --skip-existing --json
```

## Defaults

The CLI reads defaults from `~\.config\hwpc\config.json`, then environment variables, then explicit CLI flags. Use explicit CLI flags for one-off behavior because they override both config and environment.

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

Supported env defaults are `HWPC_OVERWRITE`, `HWPC_SKIP_EXISTING`, `HWPC_JSON`, `HWPC_RECURSIVE`, and `HWPC_FILE_PATH_CHECKER_DLL`. The existing `HWP_FILE_PATH_CHECKER_DLL` env var is also accepted.

## FilePathChecker DLL

Do not assume `FilePathCheckerModuleExample.dll` is bundled. It is not included in the crate or repo because redistribution rights are not established. If the user provides the DLL, register it for future conversions:

```powershell
target\release\hwpc.exe file-checker install C:\path\to\FilePathCheckerModuleExample.dll
```

This copies the DLL to the hwpc config directory and updates `config.json`.

## Agent Procedure

1. Confirm the requested target format. Supported formats are `pdf`, `hwpx`, `hwp`, `hml`, `html`, `odt`, `docx`, `txt`, and `rtf`.
2. Check that the input path exists.
3. Build the CLI if `target\release\hwpc.exe` is missing.
4. Run the conversion with `--json` so each converted file is machine-readable.
5. Report the output path(s) and any failures.

## Behavior

The CLI uses Hancom automation and calls `SaveAs(output, targetType, "")`. PDF conversion intentionally uses `SaveAs("PDF")` only; virtual PDF printer conversion is not part of this v1 skill.
