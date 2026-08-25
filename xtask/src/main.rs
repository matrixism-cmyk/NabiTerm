//! nabi xtask — 워크스페이스 자동화.
//!
//! 현재 작업:
//! - `lines` : 소스 파일 라인 수 게이트(소프트 250 경고 / 하드 400 실패).
//! - `unsafe-audit` : 모든 unsafe에 SAFETY 근거 주석이 붙어 있는지 검사.
//! - `dist`  : 정기 배포 산출물(Inno Setup 설치본) 생성.
//! - `dist-standalone` : 요청 시에만 포터블 zip 생성.
//! - `dist-mesa` : 고정 Mesa 런타임 zip 수동 생성.
//! - `icon`  : 빌드된 exe에 나비 아이콘 주입(windres 부재 환경 대응).
//! - `prerelease` : 잠금 파일에 alpha/beta/rc 의존성이 섞였는지 검사.
//! - `e2e`   : 앱을 실제로 띄워 제어 평면으로 스모크(기동→pane→입력→캡처→종료).
//! - `soak`  : 앱을 N분 굴리며 생존·응답성·메모리 관찰(수시/야간 점검).

mod dist;
mod e2e;
mod icon;
mod lines;
mod overrides;
mod prerelease;
mod soak; mod postverify;
mod releasetarget; mod unsafeaudit;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "lines" => lines::run(),
        "unsafe-audit" => unsafeaudit::run(),
        "dist" => dist::run(),
        "dist-standalone" => dist::standalone(),
        "dist-mesa" => dist::mesa(),
        "prerelease" => prerelease::run(),
        "e2e" => e2e::run(std::env::args().nth(2)),
        "soak" => soak::run(std::env::args().nth(2)),
        "verify-release" => postverify::run(std::env::args().nth(2)),
        // 릴리스 저장소는 문서가 아니라 코드에서 읽는다(releasetarget.rs 주석 참고).
        "release-repo" => releasetarget::run(),
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
            eprintln!(
                "알 수 없는 작업: '{other}'. 사용 가능: lines, prerelease, e2e, soak, dist, dist-standalone, dist-mesa, icon"
            );
            ExitCode::from(2)
        }
    }
}
