//! nabi xtask — 워크스페이스 자동화.
//!
//! 현재 작업:
//! - `lines` : 소스 파일 라인 수 게이트(소프트 250 경고 / 하드 400 실패).
//! - `dist`  : 배포 산출물(포터블 zip + Inno Setup 설치본) 생성.
//! - `icon`  : 빌드된 exe에 나비 아이콘 주입(windres 부재 환경 대응).

mod dist;
mod icon;
mod lines;
mod overrides;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "lines" => lines::run(),
        "dist" => dist::run(),
        "icon" => {
            let p = std::env::args()
                .nth(2)
                .unwrap_or_else(|| "target/release/nabi.exe".into());
            match icon::patch(std::path::Path::new(&p)) {
                Ok(()) => {
                    println!("아이콘 주입 완료: {p}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("알 수 없는 작업: '{other}'. 사용 가능: lines, dist, icon");
            ExitCode::from(2)
        }
    }
}
