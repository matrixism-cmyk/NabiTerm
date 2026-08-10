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
    ExitCode::SUCCESS
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
