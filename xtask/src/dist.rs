//! `cargo xtask dist` — 정기 배포용 설치본만 생성.
//!
//! 1) `cargo build --release`
//! 2) `dist/stage/nabiTerm.exe` 스테이징 — 설치본 프로세스명을 개발본(nabi.exe)과
//!    구별해, 개발 중 프로세스 정리가 설치본을 죽이지 않게 한다.
//!
//! 포터블과 고정 Mesa 런타임은 각각 `dist-standalone`, `dist-mesa`로 수동 생성한다.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub fn run() -> ExitCode {
    let root = workspace_root();
    if stage_release(&root, false).is_err() {
        return ExitCode::FAILURE;
    }
    build_setup(&root)
}

pub fn standalone() -> ExitCode {
    let root = workspace_root();
    if stage_release(&root, true).is_err() {
        return ExitCode::FAILURE;
    }
    build_standalone(&root)
}

pub fn mesa() -> ExitCode {
    let root = workspace_root();
    build_mesa_zip(&root);
    ExitCode::SUCCESS
}

fn stage_release(root: &Path, portable: bool) -> Result<(), ()> {
    let ok = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(root)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("release 빌드 실패");
        return Err(());
    }
    let stage = root.join("dist").join("stage");
    if std::fs::create_dir_all(&stage).is_err() {
        eprintln!("dist/stage 생성 실패");
        return Err(());
    }
    let exe = root.join("target").join("release").join("nabi.exe");
    // windres 부재 환경: 빌드 산출물에 아이콘을 사후 주입(개발본·스테이징본 공통).
    if let Err(e) = crate::icon::patch(&exe) {
        eprintln!("아이콘 주입 실패(계속): {e}");
    }
    if let Err(e) = std::fs::copy(&exe, stage.join("nabiTerm.exe")) {
        eprintln!("스테이징 실패: {e}");
        return Err(());
    }
    // exe 가 시작할 때 요구하는 DLL 을 함께 넣는다.
    //
    // 내장 웹 브라우저를 붙이면서 exe 가 WebView2Loader.dll 을 요구하게 됐는데 설치본에
    // 넣지 않아 **프로그램이 아예 뜨지 않았다**(v0.1.491, 사용자 보고). 개발 중에는 cargo 가
    // 빌드 폴더에 그 DLL 을 놓아 줘서 아무 문제가 없었다 — 그래서 아무도 몰랐다.
    copy_runtime_dlls(root, &stage)?;

    let marker = stage.join("portable.toml");
    if portable {
        let _ = std::fs::write(&marker, "# nabiTerm 포터블 모드 마커 — 설정/세션을 exe 옆에 저장한다.\n");
    } else {
        let _ = std::fs::remove_file(marker);
    }
    Ok(())
}

fn build_standalone(root: &Path) -> ExitCode {
    let stage = root.join("dist").join("stage");
    let zip = root.join("dist").join("nabiTerm-standalone.zip");
    let _ = std::fs::remove_file(&zip);
    let ps = format!(
        "Compress-Archive -Path '{}' -DestinationPath '{}'",
        stage.join("*").display(),
        zip.display()
    );
    let zipped = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .status()
        .is_ok_and(|s| s.success());
    if zipped {
        println!("생성: {}", zip.display());
        ExitCode::SUCCESS
    } else {
        eprintln!("standalone zip 생성 실패");
        ExitCode::FAILURE
    }
}

fn build_setup(root: &Path) -> ExitCode {
    // Inno Setup이 설치돼 있으면 설치본(setup.exe)까지 컴파일.
    let Some(iscc) = find_iscc() else {
        eprintln!("ISCC.exe 없음 — 정기 배포에는 설치본이 필수입니다. Inno Setup 6을 설치하세요.");
        return ExitCode::FAILURE;
    };
    let iss = root.join("installer").join("nabiTerm.iss");
    // 워크스페이스 버전을 인스톨러에 전달(자동 업데이트 버전 비교의 기준).
    let ver = workspace_version(root).unwrap_or_else(|| "0.1.0".into());
    let ok = Command::new(&iscc)
        .arg(format!("/DAppVer={ver}"))
        .arg(&iss)
        .current_dir(root)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("Inno Setup 컴파일 실패: {}", iss.display());
        return ExitCode::FAILURE;
    }
    println!("생성: {}", root.join("dist").join("nabiTerm-setup.exe").display());
    // 패키지 매니페스트도 여기서 함께 만든다.
    //
    // 따로 부르게 두면 언젠가 잊고, 잊으면 winget·Scoop 은 **옛 판을 계속 가리킨다**
    // — 릴리스는 성공했으므로 아무 경고도 없다(저장소 이름을 문서에 적었다가 일곱 판을
    // 놓친 것과 같은 결이다). 실패해도 배포를 막지는 않는다.
    if crate::pkg::run() != ExitCode::SUCCESS {
        eprintln!("경고: 패키지 매니페스트를 만들지 못했다 — `xtask pkg` 를 따로 확인할 것");
    }
    ExitCode::SUCCESS
}


/// exe 옆에 있어야 하는 DLL 들을 스테이징에 넣고, **하나라도 빠지면 빌드를 멈춘다.**
///
/// 조용히 넘어가면 뜨지 않는 설치본이 그대로 나가고, 릴리스는 성공했으므로 아무 경고도
/// 없다. 실제로 그렇게 한 판(v0.1.491)을 내보냈다.
///
/// 넣을 목록을 손으로 적어 두지 않는다 — 다음에 다른 DLL 이 늘면 또 같은 일이 난다.
/// **exe 에게 직접 물어서** 윈도우가 갖고 있지 않은 것만 골라 넣는다.
fn copy_runtime_dlls(root: &Path, stage: &Path) -> Result<(), ()> {
    let exe = stage.join("nabiTerm.exe");
    let bytes = std::fs::read(&exe).map_err(|e| eprintln!("스테이징 exe 를 읽지 못했다: {e}"))?;
    let needed = crate::pedeps::imports(&bytes).map_err(|e| eprintln!("exe 를 읽지 못했다: {e}"))?;
    let extra: Vec<&String> = needed.iter().filter(|n| !is_system_dll(n)).collect();
    if extra.is_empty() {
        return Ok(());
    }
    for dll in &extra {
        let name = dll.as_str();
        let Some(src) = find_build_dll(root, name) else {
            eprintln!("{name} 을 찾지 못했다 — 이게 없으면 설치본이 실행되지 않는다.");
            eprintln!("  찾은 곳: target/release/build/*/out/**/");
            return Err(());
        };
        // 디스크에 있는 진짜 이름으로 넣는다. exe 의 표에는 소문자로 적혀 있지만
        // 파일 이름은 WebView2Loader.dll 처럼 대소문자가 섞여 있다.
        let real = src.file_name().unwrap_or_default().to_owned();
        if let Err(e) = std::fs::copy(&src, stage.join(&real)) {
            eprintln!("{name} 복사 실패: {e}");
            return Err(());
        }
        println!("동봉: {}", real.to_string_lossy());
    }
    Ok(())
}

/// 윈도우가 이미 갖고 있는 DLL 인가. 이런 것은 우리가 넣지 않는다.
///
/// **이름 목록을 손으로 관리하지 않는다.** 처음에 쉰 개쯤 적어 뒀더니 첫 실행에서 바로
/// `uiautomationcore.dll` 이 빠져 걸렸다. 윈도우 판이 바뀌면 또 어긋난다.
///
/// 그래서 **윈도우에게 직접 묻는다** — 시스템 폴더에 그 파일이 있으면 윈도우 것이다.
///
/// `api-ms-win-*` 와 `ext-ms-*` 는 파일이 아니라 이름표다(윈도우가 속으로 다른 파일로
/// 연결해 준다). 폴더에 없지만 우리가 넣을 것도 아니므로 따로 넘긴다.
fn is_system_dll(name: &str) -> bool {
    if name.starts_with("api-ms-win-") || name.starts_with("ext-ms-") {
        return true;
    }
    let sysdir = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
    Path::new(&sysdir).join("System32").join(name).exists()
}

/// 빌드 스크립트가 놓아 둔 DLL 을 찾는다. 크레이트 폴더 이름에 해시가 붙어 훑어야 한다.
fn find_build_dll(root: &Path, name: &str) -> Option<std::path::PathBuf> {
    let build = root.join("target").join("release").join("build");
    let mut found = None;
    let mut stack = vec![build];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.eq_ignore_ascii_case(name)) {
                // x64 판을 고른다 — 같은 이름이 arm64/x86 에도 있다.
                if p.parent().is_some_and(|d| d.file_name().is_some_and(|f| f == "x64")) {
                    return Some(p);
                }
                found = found.or(Some(p));
            }
        }
    }
    found
}

/// vendor/mesa의 Mesa llvmpipe(소프트웨어 OpenGL) DLL을 별도 옵션 자산 zip으로 묶는다.
/// 메인 설치본/standalone에는 넣지 않는다(자동업데이트 가벼움 유지) — GPU 없는 VM 사용자가
/// 이 zip을 받아 nabiTerm.exe 옆에 풀면 wgpu GL 백엔드가 소프트웨어로 동작한다.
fn build_mesa_zip(root: &Path) {
    let mesa = root.join("vendor").join("mesa");
    if !mesa.join("opengl32.dll").exists() || !mesa.join("libgallium_wgl.dll").exists() {
        println!("vendor/mesa DLL 없음 — 소프트웨어 GL 자산 건너뜀.");
        return;
    }
    let zip = root.join("dist").join("nabiTerm-mesa-software-gl.zip");
    let _ = std::fs::remove_file(&zip);
    let ps = format!(
        "Compress-Archive -Path '{}' -DestinationPath '{}'",
        mesa.join("*.dll").display(),
        zip.display()
    );
    let ok = Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .status()
        .is_ok_and(|s| s.success());
    if ok {
        println!("생성: {} (별도 옵션 — GPU 없는 VM용 소프트웨어 GL)", zip.display());
    } else {
        eprintln!("Mesa 소프트웨어 GL zip 생성 실패(계속 진행)");
    }
}

/// 루트 Cargo.toml의 workspace.package.version을 읽는다.
fn workspace_version(root: &Path) -> Option<String> {
    let toml = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    // [workspace.package] 다음의 첫 version = "x.y.z".
    let after = toml.split("[workspace.package]").nth(1)?;
    after.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix("version")
            .and_then(|r| r.split('"').nth(1))
            .map(|v| v.to_string())
    })
}

/// Inno Setup 6 컴파일러 탐색(표준 설치 경로 + PATH).
fn find_iscc() -> Option<PathBuf> {
    for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(var) {
            for sub in ["Inno Setup 6\\ISCC.exe", "Programs\\Inno Setup 6\\ISCC.exe"] {
                let p = Path::new(&base).join(sub);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    // PATH에 있으면 이름만으로 실행 가능.
    Command::new("ISCC.exe")
        .arg("/?")
        .output()
        .ok()
        .map(|_| PathBuf::from("ISCC.exe"))
}

/// 워크스페이스 루트(xtask 매니페스트의 부모).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask는 워크스페이스 멤버")
        .to_path_buf()
}
