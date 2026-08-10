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

fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new(program);
    c.creation_flags(0x0800_0000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    c
}

fn run_ps(script: &str) -> std::io::Result<Output> {
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

fn set(job: &ActionJob, fraction: f32, message: impl Into<String>) {
    if let Ok(mut p) = job.lock() {
        p.fraction = fraction;
        p.message = message.into();
    }
}

fn finish(job: &ActionJob, out: std::io::Result<Output>) {
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
            ("claude", true) => {
                set(&worker, 0.4, "Claude Code 제거 중…");
                run_ps(&npm_script("uninstall -g @anthropic-ai/claude-code"))
            }
            ("codex", false) => install_codex(&worker),
            ("codex", true) => {
                set(&worker, 0.4, "Codex 제거 중…");
                run_ps(&npm_script("uninstall -g @openai/codex"))
            }
            ("antigravity", false) => {
                set(&worker, 0.3, "Antigravity 다운로드 및 설치 중…");
                run_ps("$p=Join-Path $env:TEMP 'antigravity-install.ps1'; Invoke-WebRequest https://antigravity.google/cli/install.ps1 -OutFile $p; & $p")
            }
            ("antigravity", true) => run_ps("Start-Process 'https://antigravity.google/download'"),
            _ => unreachable!(),
        };
        finish(&worker, out);
    });
    Ok(job)
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

fn install_npm_cli(job: &ActionJob, name: &str, package: &str) -> std::io::Result<Output> {
    if resolve("node").is_none() || resolve("npm").is_none() {
        set(job, 0.15, "Node.js LTS 설치 중…");
        let node = install_portable_node()?;
        if !node.status.success() {
            return Ok(node);
        }
        expose_local_tools();
    }
    set(job, 0.65, format!("{name} 설치 중…"));
    let out = run_ps(&npm_script(&format!("install -g {package}")))?;
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

/// winget·관리자 권한 없이 공식 Node.js LTS ZIP을 받고 SHASUMS256으로 검증한다.
fn install_portable_node() -> std::io::Result<Output> {
    let root = tools_root().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "LOCALAPPDATA not found")
    })?;
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let script = format!(
        r#"
$ErrorActionPreference='Stop'; $root='{root}'; $dest=Join-Path $root 'nodejs'; $npm=Join-Path $root 'npm'
$idx=Invoke-RestMethod 'https://nodejs.org/dist/index.json'; $v=($idx|Where-Object {{$_.lts}}|Select-Object -First 1).version
$name="node-$v-win-{arch}.zip"; $base="https://nodejs.org/dist/$v"; $tmp=Join-Path $env:TEMP $name
$sums=(Invoke-WebRequest "$base/SHASUMS256.txt" -UseBasicParsing).Content
$line=($sums -split "`n"|Where-Object {{$_.Trim().EndsWith($name)}}|Select-Object -First 1)
if(-not $line){{throw 'Node.js checksum missing'}}; $expected=($line.Trim() -split '\s+')[0]
Invoke-WebRequest "$base/$name" -OutFile $tmp -UseBasicParsing
if((Get-FileHash $tmp -Algorithm SHA256).Hash -ne $expected){{throw 'Node.js SHA256 mismatch'}}
$unpack=Join-Path $root '_node_unpack'; if(Test-Path $unpack){{Remove-Item $unpack -Recurse -Force}}
Expand-Archive $tmp $unpack -Force; if(Test-Path $dest){{Remove-Item $dest -Recurse -Force}}
Move-Item (Join-Path $unpack "node-$v-win-{arch}") $dest; Remove-Item $unpack -Recurse -Force; Remove-Item $tmp -Force
New-Item $npm -ItemType Directory -Force|Out-Null; $user=[Environment]::GetEnvironmentVariable('Path','User')
foreach($p in @($dest,$npm)){{if(($user -split ';') -notcontains $p){{$user="$p;$user"}}}}
[Environment]::SetEnvironmentVariable('Path',$user,'User')
"#,
        root = root.display()
    );
    run_ps(&script)
}
