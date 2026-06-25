use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SUPPORTED_FORMATS: &[(&str, &str)] = &[
    ("pdf", "PDF"),
    ("hwpx", "HWPX"),
    ("hwp", "HWP"),
    ("hml", "HWPML2X"),
    ("html", "HTML+"),
    ("odt", "ODT"),
    ("docx", "OOXML"),
    ("txt", "UNICODE"),
    ("rtf", "RTF"),
];

const SOURCE_EXTENSIONS: &[&str] = &[
    "hwp", "hwpx", "hml", "html", "htm", "odt", "docx", "doc", "txt", "rtf",
];

fn main() {
    match run(env::args().collect()) {
        Ok(()) => {}
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let cli = Cli::parse(args)?;

    match cli.command {
        CommandKind::Help => {
            print_help();
            Ok(())
        }
        CommandKind::Formats => {
            for (ext, hwp_type) in SUPPORTED_FORMATS {
                println!("{ext}\t{hwp_type}");
            }
            Ok(())
        }
        CommandKind::Convert(opts) => convert(opts),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    command: CommandKind,
}

#[derive(Debug, PartialEq, Eq)]
enum CommandKind {
    Help,
    Formats,
    Convert(ConvertOptions),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConvertOptions {
    inputs: Vec<PathBuf>,
    to: OutputFormat,
    output: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    recursive: bool,
    overwrite: bool,
    skip_existing: bool,
    json: bool,
    file_path_checker_dll: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OutputFormat {
    extension: &'static str,
    hwp_type: &'static str,
}

impl OutputFormat {
    fn parse(value: &str) -> Result<Self, String> {
        let normalized = value.trim().trim_start_matches('.').to_ascii_lowercase();
        SUPPORTED_FORMATS
            .iter()
            .find(|(ext, hwp_type)| {
                *ext == normalized || hwp_type.eq_ignore_ascii_case(normalized.as_str())
            })
            .map(|(extension, hwp_type)| Self {
                extension,
                hwp_type,
            })
            .ok_or_else(|| format!("unsupported output format `{value}`"))
    }
}

impl Cli {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let _bin = args.next();
        let Some(command) = args.next() else {
            return Ok(Self {
                command: CommandKind::Help,
            });
        };

        match command.as_str() {
            "-h" | "--help" | "help" => Ok(Self {
                command: CommandKind::Help,
            }),
            "formats" => Ok(Self {
                command: CommandKind::Formats,
            }),
            "convert" => Ok(Self {
                command: CommandKind::Convert(parse_convert(args.collect())?),
            }),
            other => Err(format!("unknown command `{other}`")),
        }
    }
}

fn parse_convert(args: Vec<String>) -> Result<ConvertOptions, String> {
    let mut inputs = Vec::new();
    let mut to = None;
    let mut output = None;
    let mut out_dir = None;
    let mut recursive = false;
    let mut overwrite = false;
    let mut skip_existing = false;
    let mut json = false;
    let mut file_path_checker_dll = env::var_os("HWP_FILE_PATH_CHECKER_DLL").map(PathBuf::from);

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--to" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "`--to` requires a value".to_string())?;
                to = Some(OutputFormat::parse(&value)?);
            }
            "--output" | "-o" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "`--output` requires a path".to_string())?;
                output = Some(PathBuf::from(value));
            }
            "--out-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "`--out-dir` requires a directory".to_string())?;
                out_dir = Some(PathBuf::from(value));
            }
            "--recursive" | "-r" => recursive = true,
            "--overwrite" => overwrite = true,
            "--skip-existing" => skip_existing = true,
            "--json" => json = true,
            "--file-path-checker-dll" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "`--file-path-checker-dll` requires a path".to_string())?;
                file_path_checker_dll = Some(PathBuf::from(value));
            }
            "-h" | "--help" => {
                return Err("use `hwp-convert-cli help` for usage".to_string());
            }
            value if value.starts_with('-') => return Err(format!("unknown option `{value}`")),
            value => inputs.push(PathBuf::from(value)),
        }
    }

    if inputs.is_empty() {
        return Err("convert requires at least one input file or directory".to_string());
    }
    if output.is_some() && out_dir.is_some() {
        return Err("use either `--output` or `--out-dir`, not both".to_string());
    }
    if output.is_some() && inputs.len() > 1 {
        return Err("`--output` can only be used with one input".to_string());
    }
    if overwrite && skip_existing {
        return Err("use either `--overwrite` or `--skip-existing`, not both".to_string());
    }

    Ok(ConvertOptions {
        inputs,
        to: to.ok_or_else(|| "`--to <FORMAT>` is required".to_string())?,
        output,
        out_dir,
        recursive,
        overwrite,
        skip_existing,
        json,
        file_path_checker_dll,
    })
}

fn convert(opts: ConvertOptions) -> Result<(), String> {
    require_windows()?;
    let jobs = collect_jobs(&opts)?;

    if jobs.is_empty() {
        return Err("no convertible input files found".to_string());
    }

    let mut failures = 0;
    for job in jobs {
        let status = if job.output.exists() && opts.skip_existing {
            ConvertStatus::SkippedExisting
        } else {
            if job.output.exists() && !opts.overwrite {
                return Err(format!(
                    "output already exists: {} (use --overwrite or --skip-existing)",
                    job.output.display()
                ));
            }
            run_hancom_save_as(
                &job.input,
                &job.output,
                opts.to,
                opts.file_path_checker_dll.as_deref(),
            )?
        };

        if opts.json {
            println!(
                "{{\"input\":\"{}\",\"output\":\"{}\",\"format\":\"{}\",\"status\":\"{}\"}}",
                json_escape(&job.input.display().to_string()),
                json_escape(&job.output.display().to_string()),
                opts.to.extension,
                status.as_str()
            );
        } else {
            println!(
                "{} -> {} [{}]",
                job.input.display(),
                job.output.display(),
                status.as_str()
            );
        }

        if status == ConvertStatus::Failed {
            failures += 1;
        }
    }

    if failures > 0 {
        Err(format!("{failures} conversion(s) failed"))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn require_windows() -> Result<(), String> {
    Err("Hancom Office automation is only available on Windows".to_string())
}

#[cfg(windows)]
fn require_windows() -> Result<(), String> {
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct ConvertJob {
    input: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConvertStatus {
    Converted,
    SkippedExisting,
    Failed,
}

impl ConvertStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Converted => "converted",
            Self::SkippedExisting => "skipped-existing",
            Self::Failed => "failed",
        }
    }
}

fn collect_jobs(opts: &ConvertOptions) -> Result<Vec<ConvertJob>, String> {
    let mut files = Vec::new();
    for input in &opts.inputs {
        collect_input(input, opts.recursive, &mut files)?;
    }
    files.sort();
    files.dedup();

    if opts.output.is_some() && files.len() > 1 {
        return Err("`--output` can only be used when exactly one input file is found".to_string());
    }

    let mut jobs = Vec::new();
    for input in files {
        let output = resolve_output_path(&input, opts)?;
        if same_path(&input, &output) {
            return Err(format!(
                "input and output resolve to the same file: {}",
                input.display()
            ));
        }
        jobs.push(ConvertJob { input, output });
    }
    Ok(jobs)
}

fn collect_input(path: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if is_convertible_source(path) {
            files.push(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
        }
        return Ok(());
    }

    if path.is_dir() {
        if !recursive {
            return Err(format!(
                "{} is a directory; pass --recursive to scan it",
                path.display()
            ));
        }
        for entry in fs::read_dir(path).map_err(|err| format!("{}: {err}", path.display()))? {
            let entry = entry.map_err(|err| err.to_string())?;
            collect_input(&entry.path(), recursive, files)?;
        }
        return Ok(());
    }

    Err(format!("input does not exist: {}", path.display()))
}

fn is_convertible_source(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            SOURCE_EXTENSIONS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(ext))
        })
        .unwrap_or(false)
}

fn resolve_output_path(input: &Path, opts: &ConvertOptions) -> Result<PathBuf, String> {
    if let Some(output) = &opts.output {
        return Ok(output.clone());
    }

    let file_stem = input
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("cannot determine file name for {}", input.display()))?;
    let file_name = format!("{file_stem}.{}", opts.to.extension);

    if let Some(out_dir) = &opts.out_dir {
        return Ok(out_dir.join(file_name));
    }

    Ok(input.with_file_name(file_name))
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => a == b,
    }
}

fn run_hancom_save_as(
    input: &Path,
    output: &Path,
    format: OutputFormat,
    file_path_checker_dll: Option<&Path>,
) -> Result<ConvertStatus, String> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    let script = build_powershell_script(
        &input
            .canonicalize()
            .map_err(|err| format!("{}: {err}", input.display()))?,
        output,
        format,
        file_path_checker_dll,
    );
    let mut child = Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to start powershell.exe: {err}"))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "failed to open PowerShell stdin".to_string())?;
        stdin
            .write_all(script.as_bytes())
            .map_err(|err| format!("failed to write PowerShell script: {err}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for PowerShell: {err}"))?;

    if output.status.success() {
        Ok(ConvertStatus::Converted)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stderr.trim().is_empty() {
            eprintln!("{}", stderr.trim());
        }
        if !stdout.trim().is_empty() {
            eprintln!("{}", stdout.trim());
        }
        Ok(ConvertStatus::Failed)
    }
}

fn build_powershell_script(
    input: &Path,
    output: &Path,
    format: OutputFormat,
    file_path_checker_dll: Option<&Path>,
) -> String {
    let checker = file_path_checker_dll
        .map(|path| ps_string(&path.display().to_string()))
        .unwrap_or_else(|| "$null".to_string());
    format!(
        r#"
$ErrorActionPreference = 'Stop'
$inputPath = {input}
$outputPath = {output}
$targetType = {target_type}
$checkerDll = {checker}
$hwp = $null
try {{
    $hwp = New-Object -ComObject HWPFrame.HwpObject
    $registered = $false
    if ($checkerDll -ne $null -and $checkerDll.Length -gt 0) {{
        if (-not (Test-Path -LiteralPath $checkerDll)) {{
            throw "FilePathChecker DLL not found: $checkerDll"
        }}
        $moduleKey = 'HKCU:\Software\HNC\HwpAutomation\Modules'
        if (-not (Test-Path -LiteralPath $moduleKey)) {{
            New-Item -Path $moduleKey -Force | Out-Null
        }}
        New-ItemProperty -Path $moduleKey -Name 'FilePathCheckerModuleExample' -Value $checkerDll -PropertyType String -Force | Out-Null
        $registered = [bool]$hwp.RegisterModule('FilePathCheckDLL', 'FilePathCheckerModuleExample')
        if ($registered) {{
            $hwp.SetMessageBoxMode(0x00211411) | Out-Null
        }}
    }}
    $opened = [bool]$hwp.Open($inputPath, '', 'lock:false;forceopen:true;suspendpassword:true;')
    if (-not $opened) {{
        throw "failed to open input: $inputPath"
    }}
    $saved = [bool]$hwp.SaveAs($outputPath, $targetType, '')
    if (-not $saved) {{
        throw "SaveAs failed for target type $targetType"
    }}
    if (-not (Test-Path -LiteralPath $outputPath)) {{
        throw "SaveAs reported success but output was not created: $outputPath"
    }}
    exit 0
}} catch {{
    Write-Error $_
    exit 1
}} finally {{
    if ($hwp -ne $null) {{
        try {{ $hwp.Clear(1) | Out-Null }} catch {{ }}
        try {{ $hwp.Quit() | Out-Null }} catch {{ }}
        [System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($hwp) | Out-Null
    }}
}}
"#,
        input = ps_string(&input.display().to_string()),
        output = ps_string(&output.display().to_string()),
        target_type = ps_string(format.hwp_type),
        checker = checker,
    )
}

fn ps_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            other if other < '\u{20}' => {
                escaped.push_str(&format!("\\u{:04x}", other as u32));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

fn print_help() {
    println!(
        r#"hwp-convert-cli

Convert Hancom document files by automating installed Hancom Office on Windows.

USAGE:
  hwp-convert-cli convert <INPUT...> --to <FORMAT> [OPTIONS]
  hwp-convert-cli formats

FORMATS:
  pdf, hwpx, hwp, hml, html, odt, docx, txt, rtf

OPTIONS:
  --to <FORMAT>                  Output format, e.g. pdf or hwpx
  -o, --output <PATH>            Output file path; only valid for one input
  --out-dir <DIR>                Directory for converted files
  -r, --recursive                Recursively scan input directories
  --overwrite                    Replace existing output files
  --skip-existing                Skip files whose output already exists
  --json                         Print one JSON object per input
  --file-path-checker-dll <PATH> Register Hancom FilePathCheckerModuleExample.dll

ENV:
  HWP_FILE_PATH_CHECKER_DLL       Default FilePathCheckerModuleExample.dll path
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_convert_command_for_pdf() {
        let cli = Cli::parse(vec![
            "hwp-convert-cli".into(),
            "convert".into(),
            "sample.hwp".into(),
            "--to".into(),
            "pdf".into(),
            "--json".into(),
        ])
        .unwrap();

        assert_eq!(
            cli,
            Cli {
                command: CommandKind::Convert(ConvertOptions {
                    inputs: vec![PathBuf::from("sample.hwp")],
                    to: OutputFormat {
                        extension: "pdf",
                        hwp_type: "PDF"
                    },
                    output: None,
                    out_dir: None,
                    recursive: false,
                    overwrite: false,
                    skip_existing: false,
                    json: true,
                    file_path_checker_dll: env::var_os("HWP_FILE_PATH_CHECKER_DLL")
                        .map(PathBuf::from),
                })
            }
        );
    }

    #[test]
    fn rejects_output_with_multiple_inputs() {
        let err = parse_convert(vec![
            "a.hwp".into(),
            "b.hwp".into(),
            "--to".into(),
            "pdf".into(),
            "--output".into(),
            "out.pdf".into(),
        ])
        .unwrap_err();

        assert!(err.contains("only be used with one input"));
    }

    #[test]
    fn resolves_default_output_path_next_to_input() {
        let opts = ConvertOptions {
            inputs: vec![PathBuf::from("report.hwp")],
            to: OutputFormat {
                extension: "hwpx",
                hwp_type: "HWPX",
            },
            output: None,
            out_dir: None,
            recursive: false,
            overwrite: false,
            skip_existing: false,
            json: false,
            file_path_checker_dll: None,
        };

        assert_eq!(
            resolve_output_path(Path::new("C:\\docs\\report.hwp"), &opts).unwrap(),
            PathBuf::from("C:\\docs\\report.hwpx")
        );
    }

    #[test]
    fn escapes_powershell_single_quotes() {
        assert_eq!(ps_string("C:\\a'b\\x.hwp"), "'C:\\a''b\\x.hwp'");
    }

    #[test]
    fn escapes_json_strings() {
        assert_eq!(
            json_escape("C:\\docs\\\"a\".hwp"),
            "C:\\\\docs\\\\\\\"a\\\".hwp"
        );
        assert_eq!(json_escape("a\u{01}b\u{08}c\u{0c}"), "a\\u0001b\\bc\\f");
    }

    #[test]
    fn output_rejects_more_than_one_collected_file() {
        let root = env::temp_dir().join(format!(
            "hwp2hwpx-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.hwp"), b"").unwrap();
        fs::write(root.join("b.hwp"), b"").unwrap();

        let opts = ConvertOptions {
            inputs: vec![root.clone()],
            to: OutputFormat {
                extension: "pdf",
                hwp_type: "PDF",
            },
            output: Some(root.join("out.pdf")),
            out_dir: None,
            recursive: true,
            overwrite: false,
            skip_existing: false,
            json: false,
            file_path_checker_dll: None,
        };

        let err = collect_jobs(&opts).unwrap_err();
        assert!(err.contains("exactly one input file"));

        fs::remove_dir_all(root).unwrap();
    }
}
