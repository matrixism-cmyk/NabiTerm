//! 배포 산출물 검증(verify-release) — dist의 standalone zip을 풀어 **실제 배포 바이너리**를
//! 별도 프로세스로 격리 실행하고, 제어평면 스모크 + 종료 후 로그 오류 스캔까지 한다.
//!
//! 사용: `cargo run -p xtask -- verify-release [zip경로]` (기본 dist\nabiTerm-standalone.zip)
//! 릴리스 절차의 마지막 게이트: 릴리스 후 이 명령으로 "설치된 것과 같은 바이너리"를 검증하고,
//! 로그에 ERROR/panic이 있으면 실패로 알려 후속 수정으로 잇는다(사용자 요청 2026-08-16).

use std::process::ExitCode;

pub fn run(zip: Option<String>) -> ExitCode {
    match verify(zip) {
        Ok(()) => {
            println!("배포 산출물 검증 통과 (스모크+로그 스캔)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("배포 산출물 검증 실패: {e}");
            ExitCode::FAILURE
        }
    }
}

fn verify(zip: Option<String>) -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let zip = zip.map(std::path::PathBuf::from).unwrap_or(root.join("dist/nabiTerm-standalone.zip"));
    if !zip.exists() {
        return Err(format!("zip 없음: {} (xtask dist 먼저)", zip.display()));
    }
    let out = std::env::temp_dir().join(format!("nabi-postverify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    // Windows 내장 PowerShell로 해제(별도 zip 의존성 없이 — dist의 압축 방식과 대칭).
    let st = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Expand-Archive -Force -LiteralPath"])
        .arg(&zip)
        .args(["-DestinationPath"])
        .arg(&out)
        .status()
        .map_err(|e| format!("Expand-Archive 실행 실패: {e}"))?;
    if !st.success() {
        return Err("zip 해제 실패".into());
    }
    let exe = find_exe(&out).ok_or("압축 안에 nabi.exe 없음")?;
    println!("검증 대상: {}", exe.display());
    let result = crate::e2e::smoke(Some(exe.to_string_lossy().into_owned()));
    let _ = std::fs::remove_dir_all(&out);
    result
}

/// 하위 폴더 포함 nabi.exe 탐색(zip 루트 구조가 바뀌어도 견딤).
fn find_exe(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find_exe(&p) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|n| {
            n.eq_ignore_ascii_case("nabi.exe") || n.eq_ignore_ascii_case("nabiTerm.exe")
        }) {
            return Some(p);
        }
    }
    None
}
