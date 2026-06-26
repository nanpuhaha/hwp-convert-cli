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

## Agent Procedure

1. Confirm the requested target format. Supported formats are `pdf`, `hwpx`, `hwp`, `hml`, `html`, `odt`, `docx`, `txt`, and `rtf`.
2. Check that the input path exists.
3. Build the CLI if `target\release\hwpc.exe` is missing.
4. Run the conversion with `--json` so each converted file is machine-readable.
5. Report the output path(s) and any failures.

## Behavior

The CLI uses Hancom automation and calls `SaveAs(output, targetType, "")`. PDF conversion intentionally uses `SaveAs("PDF")` only; virtual PDF printer conversion is not part of this v1 skill.
