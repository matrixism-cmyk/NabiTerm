//! AI CLI 설치 탐지와 창 없는 자동 설치 작업.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub(crate) struct CliStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
}

impl CliStatus {
    pub fn installed(&self) -> bool {
        self.path.is_some()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ActionProgress {
    pub fraction: f32,
    pub message: String,
    pub done: bool,
    pub success: bool,
    pub refresh_done: bool,
}

pub(crate) type ActionJob = Arc<Mutex<ActionProgress>>;

pub(crate) fn detect_all() -> Vec<CliStatus> {
    [
        ("claude", "Claude Code", "claude"),
        ("codex", "OpenAI Codex", "codex"),
        ("antigravity", "Google Antigravity", "agy"),
    ]
    .into_iter()
    .map(|(id, name, command)| detect(id, name, command))
    .collect()
}

fn detect(id: &'static str, name: &'static str, command: &'static str) -> CliStatus {
    let path = resolve(command);
    let version = path
        .as_ref()
        .and_then(|p| hidden(p).arg("--version").output().ok())
        .filter(|o| o.status.success())
        .and_then(first_output_line);
    CliStatus {
        id,
        name,
        command,
        path,
        version,
    }
}

fn first_output_line(o: Output) -> Option<String> {
    let bytes = if o.stdout.is_empty() {
        o.stderr
    } else {
        o.stdout
    };
    String::from_utf8_lossy(&bytes)
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn resolve(command: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .collect();
    for var in ["APPDATA", "LOCALAPPDATA", "ProgramFiles", "USERPROFILE"] {
        if let Some(p) = std::env::var_os(var) {
            let p = PathBuf::from(p);
            dirs.extend([
                p.join("npm"),
                p.join("Programs").join("nodejs"),
                p.join("nodejs"),
                p.join(".local").join("bin"),
            ]);
        }
    }
    if let Some(root) = tools_root() {
        dirs.extend([root.join("nodejs"), root.join("npm")]);
    }
    dirs.into_iter()
        .flat_map(|d| ["exe", "cmd", "bat"].map(move |e| d.join(format!("{command}.{e}"))))
        .find(|p| p.is_file())
}

pub(crate) fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(program);
    c.creation_flags(0x0800_0000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

pub(crate) fn run_ps(script: &str) -> std::io::Result<Output> {
    hidden("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
}

pub(crate) fn set_progress(job: &ActionJob, fraction: f32, message: impl Into<String>) {
    if let Ok(mut p) = job.lock() {
        p.fraction = fraction;
        p.message = message.into();
    }
}

pub(crate) fn finish(job: &ActionJob, out: std::io::Result<Output>) {
    let (success, msg) = match out {
        Ok(o) if o.status.success() => (true, "완료".to_string()),
        Ok(o) => (
            false,
            first_output_line(o).unwrap_or_else(|| "설치 명령 실패".into()),
        ),
        Err(e) => (false, e.to_string()),
    };
    if let Ok(mut p) = job.lock() {
        p.fraction = 1.0;
        p.message = msg;
        p.done = true;
        p.success = success;
    }
}

/// 별도 콘솔 창 없이 백그라운드에서 설치/제거한다. Codex는 Node.js LTS를 먼저 보장한다.
pub(crate) fn start_action(id: &str, remove: bool) -> std::io::Result<ActionJob> {
    if !matches!(id, "claude" | "codex" | "antigravity") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unknown AI CLI",
        ));
    }
    let id = id.to_string();
    let job = Arc::new(Mutex::new(ActionProgress {
        message: "준비 중…".into(),
        ..Default::default()
    }));
    let worker = job.clone();
    std::thread::spawn(move || {
        let out = match (id.as_str(), remove) {
            ("claude", false) => {
                install_npm_cli(&worker, "Claude Code", "@anthropic-ai/claude-code@latest")
            }
            ("claude", true) => remove_npm(&worker, "Claude Code", "@anthropic-ai/claude-code"),
            ("codex", false) => install_codex(&worker),
            ("codex", true) => remove_npm(&worker, "Codex", "@openai/codex"),
            ("antigravity", false) => install_antigravity(&worker),
            ("antigravity", true) => run_ps("Start-Process 'https://antigravity.google/download'"),
            _ => unreachable!(),
        };
        finish(&worker, out);
    });
    Ok(job)
}

/// npm 전역 제거 — 짧지만 그래도 출력을 흘려 읽어 막대가 살아 있게 한다.
fn remove_npm(job: &ActionJob, name: &str, package: &str) -> std::io::Result<Output> {
    let script = npm_script(&format!("uninstall -g {package}"));
    crate::aiclirun::run_ps(job, &script, 0.05, 0.95, &format!("{name} 제거 중"))
}

/// Antigravity 설치 — 내려받기와 설치를 나눠 두 단계로 보여 준다.
///
/// `Invoke-WebRequest`는 자기 진행률을 stdout에 흘리지 않으므로, 스크립트가 단계마다 한 줄씩
/// 찍게 해서 최소한 어디까지 왔는지는 보이게 한다.
fn install_antigravity(job: &ActionJob) -> std::io::Result<Output> {
    let script = "$ProgressPreference='Continue'; \
        $p=Join-Path $env:TEMP 'antigravity-install.ps1'; \
        Write-Output 'downloading installer'; \
        Invoke-WebRequest https://antigravity.google/cli/install.ps1 -OutFile $p; \
        Write-Output 'running installer'; \
        & $p";
    crate::aiclirun::run_ps(job, script, 0.05, 0.95, "Antigravity 설치 중")
}

fn npm_script(args: &str) -> String {
    let local = tools_root().map(|r| r.join("nodejs").join("npm.cmd"));
    let call = match local.filter(|p| p.is_file()) {
        Some(p) => {
            let prefix = tools_root()
                .expect("local node requires tools root")
                .join("npm");
            format!("& '{}' {args} --prefix '{}'", p.display(), prefix.display())
        }
        None => format!("npm.cmd {args}"),
    };
    format!("$env:Path=[Environment]::GetEnvironmentVariable('Path','Machine')+';'+[Environment]::GetEnvironmentVariable('Path','User'); {call}")
}

fn install_codex(job: &ActionJob) -> std::io::Result<Output> {
    install_npm_cli(job, "OpenAI Codex", "@openai/codex@latest")
}

pub(crate) fn install_npm_cli(job: &ActionJob, name: &str, package: &str) -> std::io::Result<Output> {
    // 단계 구간을 나눠 둔다. Node.js를 새로 받아야 하면 그 쪽이 시간의 절반쯤을 먹는다.
    let need_node = resolve("node").is_none() || resolve("npm").is_none();
    let split = if need_node { 0.45 } else { 0.05 };
    if need_node {
        let node = crate::aiclirun::run_ps(job, &node_script(), 0.03, split, "Node.js LTS 설치 중")?;
        if !node.status.success() {
            return Ok(node);
        }
        expose_local_tools();
    }
    let script = npm_script(&format!("install -g {package}"));
    let out = crate::aiclirun::run_ps(job, &script, split, 0.97, &format!("{name} 설치 중"))?;
    if out.status.success() {
        expose_local_tools();
    }
    Ok(out)
}

fn tools_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("nabiTerm").join("tools"))
}

/// 새 터미널뿐 아니라 현재 nabiTerm 프로세스가 만드는 터미널에도 즉시 노출한다.
fn expose_local_tools() {
    let Some(root) = tools_root() else { return };
    let mut paths = vec![root.join("nodejs"), root.join("npm")];
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    paths.dedup();
    if let Ok(joined) = std::env::join_paths(paths) {
        // SAFETY: nabiTerm does not concurrently read environment variables from Rust code here.
        unsafe { std::env::set_var("PATH", joined) };
    }
}

/// winget·관리자 권한 없이 공식 Node.js LTS ZIP을 받고 SHASUMS256으로 검증하는 스크립트.
///
/// 단계마다 한 줄씩 찍는다 — 그래야 진행 막대가 "지금 뭘 하는 중"인지 말할 수 있다.
/// 이 스크립트만 만들고 실행은 [`crate::aiclirun::run_ps`]가 맡는다(출력을 흘려 읽으려고).
fn node_script() -> String {
    let Some(root) = tools_root() else {
        return "throw 'LOCALAPPDATA not found'".to_string();
    };
    let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
    format!(
        r#"
$ErrorActionPreference='Stop'; $root='{root}'; $dest=Join-Path $root 'nodejs'; $npm=Join-Path $root 'npm'
Write-Output 'looking up the current LTS'
$idx=Invoke-RestMethod 'https://nodejs.org/dist/index.json'; $v=($idx|Where-Object {{$_.lts}}|Select-Object -First 1).version
$name="node-$v-win-{arch}.zip"; $base="https://nodejs.org/dist/$v"; $tmp=Join-Path $env:TEMP $name
Write-Output "checking the signature for $v"
$sums=(Invoke-WebRequest "$base/SHASUMS256.txt" -UseBasicParsing).Content
$line=($sums -split "`n"|Where-Object {{$_.Trim().EndsWith($name)}}|Select-Object -First 1)
if(-not $line){{throw 'Node.js checksum missing'}}; $expected=($line.Trim() -split '\s+')[0]
Write-Output "downloading $name"
Invoke-WebRequest "$base/$name" -OutFile $tmp -UseBasicParsing
Write-Output 'verifying the download'
if((Get-FileHash $tmp -Algorithm SHA256).Hash -ne $expected){{throw 'Node.js SHA256 mismatch'}}
Write-Output 'unpacking'
$unpack=Join-Path $root '_node_unpack'; if(Test-Path $unpack){{Remove-Item $unpack -Recurse -Force}}
Expand-Archive $tmp $unpack -Force; if(Test-Path $dest){{Remove-Item $dest -Recurse -Force}}
Move-Item (Join-Path $unpack "node-$v-win-{arch}") $dest; Remove-Item $unpack -Recurse -Force; Remove-Item $tmp -Force
Write-Output 'registering on PATH'
New-Item $npm -ItemType Directory -Force|Out-Null; $user=[Environment]::GetEnvironmentVariable('Path','User')
foreach($p in @($dest,$npm)){{if(($user -split ';') -notcontains $p){{$user="$p;$user"}}}}
[Environment]::SetEnvironmentVariable('Path',$user,'User')
Write-Output 'done'
"#,
        root = root.display()
    )
}

