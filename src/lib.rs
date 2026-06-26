use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

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

pub fn main_entry() {
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
            print_help(&cli.bin_name);
            Ok(())
        }
        CommandKind::Formats => {
            for (ext, hwp_type) in SUPPORTED_FORMATS {
                println!("{ext}\t{hwp_type}");
            }
            Ok(())
        }
        CommandKind::Convert(opts) => convert(opts),
        CommandKind::FileCheckerInstall(opts) => install_file_checker(opts),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    bin_name: String,
    command: CommandKind,
}

#[derive(Debug, PartialEq, Eq)]
enum CommandKind {
    Help,
    Formats,
    Convert(ConvertOptions),
    FileCheckerInstall(FileCheckerInstallOptions),
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileCheckerInstallOptions {
    source: PathBuf,
    force: bool,
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
        let bin_name = args
            .next()
            .and_then(|arg| {
                Path::new(&arg)
                    .file_stem()
                    .and_then(OsStr::to_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "hwpc".to_string());
        let Some(command) = args.next() else {
            return Ok(Self {
                bin_name,
                command: CommandKind::Help,
            });
        };

        match command.as_str() {
            "-h" | "--help" | "help" => Ok(Self {
                bin_name,
                command: CommandKind::Help,
            }),
            "formats" => Ok(Self {
                bin_name,
                command: CommandKind::Formats,
            }),
            "convert" => Ok(Self {
                bin_name,
                command: CommandKind::Convert(parse_convert(args.collect())?),
            }),
            "file-checker" => Ok(Self {
                bin_name,
                command: CommandKind::FileCheckerInstall(parse_file_checker(args.collect())?),
            }),
            other => Err(format!("unknown command `{other}`")),
        }
    }
}

fn parse_file_checker(args: Vec<String>) -> Result<FileCheckerInstallOptions, String> {
    let mut iter = args.into_iter();
    let Some(subcommand) = iter.next() else {
        return Err("usage: hwpc file-checker install <DLL_PATH> [--force]".to_string());
    };
    if subcommand != "install" {
        return Err(format!(
            "unknown file-checker subcommand `{subcommand}`; expected `install`"
        ));
    }

    let mut source = None;
    let mut force = false;
    for arg in iter {
        match arg.as_str() {
            "--force" => force = true,
            "-h" | "--help" => {
                return Err("usage: hwpc file-checker install <DLL_PATH> [--force]".to_string());
            }
            value if value.starts_with('-') => return Err(format!("unknown option `{value}`")),
            value => {
                if source.is_some() {
                    return Err("file-checker install accepts exactly one DLL path".to_string());
                }
                source = Some(PathBuf::from(value));
            }
        }
    }

    Ok(FileCheckerInstallOptions {
        source: source.ok_or_else(|| "file-checker install requires a DLL path".to_string())?,
        force,
    })
}

fn parse_convert(args: Vec<String>) -> Result<ConvertOptions, String> {
    parse_convert_with_defaults(args, DefaultOptions::load()?)
}

fn parse_convert_with_defaults(
    args: Vec<String>,
    defaults: DefaultOptions,
) -> Result<ConvertOptions, String> {
    let mut inputs = Vec::new();
    let mut to = None;
    let mut output = None;
    let mut out_dir = None;
    let mut recursive = defaults.recursive;
    let mut overwrite = defaults.overwrite;
    let mut skip_existing = defaults.skip_existing;
    let mut json = defaults.json;
    let mut file_path_checker_dll = defaults.file_path_checker_dll;

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
            "--no-recursive" => recursive = false,
            "--overwrite" => overwrite = true,
            "--no-overwrite" => overwrite = false,
            "--skip-existing" => skip_existing = true,
            "--no-skip-existing" => skip_existing = false,
            "--json" => json = true,
            "--no-json" => json = false,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DefaultOptions {
    recursive: bool,
    overwrite: bool,
    skip_existing: bool,
    json: bool,
    file_path_checker_dll: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct ConfigFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    recursive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overwrite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_existing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    json: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path_checker_dll: Option<PathBuf>,
}

impl DefaultOptions {
    fn load() -> Result<Self, String> {
        let mut defaults = Self::default();

        if let Some(path) = config_path()
            && path.exists()
        {
            let config_text = fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            let config: ConfigFile = serde_json::from_str(&config_text)
                .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
            defaults.apply_config(config);
        }

        defaults.apply_env()?;
        Ok(defaults)
    }

    fn apply_config(&mut self, config: ConfigFile) {
        if let Some(value) = config.recursive {
            self.recursive = value;
        }
        if let Some(value) = config.overwrite {
            self.overwrite = value;
        }
        if let Some(value) = config.skip_existing {
            self.skip_existing = value;
        }
        if let Some(value) = config.json {
            self.json = value;
        }
        if let Some(value) = config.file_path_checker_dll {
            self.file_path_checker_dll = Some(value);
        }
    }

    fn apply_env(&mut self) -> Result<(), String> {
        if let Some(value) = env_bool("HWPC_RECURSIVE")? {
            self.recursive = value;
        }
        if let Some(value) = env_bool("HWPC_OVERWRITE")? {
            self.overwrite = value;
        }
        if let Some(value) = env_bool("HWPC_SKIP_EXISTING")? {
            self.skip_existing = value;
        }
        if let Some(value) = env_bool("HWPC_JSON")? {
            self.json = value;
        }
        if let Some(value) = env::var_os("HWPC_FILE_PATH_CHECKER_DLL")
            .or_else(|| env::var_os("HWP_FILE_PATH_CHECKER_DLL"))
        {
            self.file_path_checker_dll = Some(PathBuf::from(value));
        }
        Ok(())
    }
}

fn config_path() -> Option<PathBuf> {
    env::var_os("HWPC_CONFIG")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".config").join("hwpc").join("config.json")))
}

fn config_dir() -> Result<PathBuf, String> {
    let path = config_path().ok_or_else(|| {
        "cannot determine config path; set HWPC_CONFIG or USERPROFILE/HOME".to_string()
    })?;
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("config path has no parent directory: {}", path.display()))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
}

fn env_bool(name: &str) -> Result<Option<bool>, String> {
    let Some(value) = env::var_os(name) else {
        return Ok(None);
    };
    let value = value.to_string_lossy();
    parse_bool(&value)
        .map(Some)
        .ok_or_else(|| format!("{name} must be one of true, false, 1, 0, yes, no, on, off"))
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
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

fn install_file_checker(opts: FileCheckerInstallOptions) -> Result<(), String> {
    if !opts.source.is_file() {
        return Err(format!(
            "DLL path does not exist: {}",
            opts.source.display()
        ));
    }
    if !opts
        .source
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("dll"))
        .unwrap_or(false)
    {
        return Err(format!("expected a .dll file: {}", opts.source.display()));
    }

    let config_dir = config_dir()?;
    let config_path =
        config_path().ok_or_else(|| "cannot determine config path for install".to_string())?;
    let target = config_dir.join("FilePathCheckerModuleExample.dll");
    fs::create_dir_all(&config_dir)
        .map_err(|err| format!("failed to create {}: {err}", config_dir.display()))?;

    let source = opts
        .source
        .canonicalize()
        .map_err(|err| format!("{}: {err}", opts.source.display()))?;
    let target_exists = target.exists();
    let same_file = if target_exists {
        same_path(&source, &target)
    } else {
        false
    };

    if target_exists && !same_file && !opts.force {
        return Err(format!(
            "{} already exists; pass --force to replace it",
            target.display()
        ));
    }
    if !same_file {
        fs::copy(&source, &target).map_err(|err| {
            format!(
                "failed to copy {} to {}: {err}",
                source.display(),
                target.display()
            )
        })?;
    }

    let mut config = read_config_file(&config_path)?;
    config.file_path_checker_dll = Some(target.clone());
    write_config_file(&config_path, &config)?;

    println!("installed {}", target.display());
    println!("updated {}", config_path.display());
    Ok(())
}

fn read_config_file(path: &Path) -> Result<ConfigFile, String> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }
    let config_text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&config_text)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn write_config_file(path: &Path, config: &ConfigFile) -> Result<(), String> {
    let config_text = serde_json::to_string_pretty(config)
        .map_err(|err| format!("failed to serialize config: {err}"))?;
    fs::write(path, format!("{config_text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
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

fn print_help(bin_name: &str) {
    println!(
        r#"{bin_name}

Convert Hancom document files by automating installed Hancom Office on Windows.

USAGE:
  {bin_name} convert <INPUT...> --to <FORMAT> [OPTIONS]
  {bin_name} formats
  {bin_name} file-checker install <DLL_PATH> [--force]

ALIASES:
  hwp-convert-cli, hwpc

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
  --no-recursive                 Disable recursive scanning from defaults
  --no-overwrite                 Disable overwrite from defaults
  --no-skip-existing             Disable skip-existing from defaults
  --no-json                      Disable JSON output from defaults

FILE CHECKER:
  The FilePathCheckerModuleExample.dll is not bundled. If you have it, install
  it into hwpc's config directory and update config.json with:
    {bin_name} file-checker install C:\path\FilePathCheckerModuleExample.dll

ENV:
  HWPC_CONFIG                    Config file path; defaults to ~/.config/hwpc/config.json
  HWPC_OVERWRITE                 Default --overwrite value
  HWPC_SKIP_EXISTING             Default --skip-existing value
  HWPC_JSON                      Default --json value
  HWPC_RECURSIVE                 Default --recursive value
  HWPC_FILE_PATH_CHECKER_DLL     Default FilePathCheckerModuleExample.dll path
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
                bin_name: "hwp-convert-cli".to_string(),
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
                    file_path_checker_dll: DefaultOptions::load().unwrap().file_path_checker_dll,
                })
            }
        );
    }

    #[test]
    fn rejects_output_with_multiple_inputs() {
        let err = parse_convert_with_defaults(
            vec![
                "a.hwp".into(),
                "b.hwp".into(),
                "--to".into(),
                "pdf".into(),
                "--output".into(),
                "out.pdf".into(),
            ],
            DefaultOptions::default(),
        )
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
    fn config_defaults_can_be_overridden_by_cli_flags() {
        let opts = parse_convert_with_defaults(
            vec![
                "sample.hwp".into(),
                "--to".into(),
                "pdf".into(),
                "--no-overwrite".into(),
                "--json".into(),
            ],
            DefaultOptions {
                overwrite: true,
                skip_existing: false,
                recursive: false,
                json: false,
                file_path_checker_dll: Some(PathBuf::from("checker.dll")),
            },
        )
        .unwrap();

        assert!(!opts.overwrite);
        assert!(opts.json);
        assert_eq!(
            opts.file_path_checker_dll,
            Some(PathBuf::from("checker.dll"))
        );
    }

    #[test]
    fn parses_bool_environment_values() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("off"), Some(false));
        assert_eq!(parse_bool("wat"), None);
    }

    #[test]
    fn parses_file_checker_install_command() {
        let opts = parse_file_checker(vec![
            "install".into(),
            "FilePathCheckerModuleExample.dll".into(),
            "--force".into(),
        ])
        .unwrap();

        assert_eq!(
            opts,
            FileCheckerInstallOptions {
                source: PathBuf::from("FilePathCheckerModuleExample.dll"),
                force: true
            }
        );
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
