//! rust-analyzer 실서버 통합(T6-4) — 이 머신에 rust-analyzer가 있을 때만 의미 있는 검증.
//! 게이트 제외(#[ignore]): `cargo test -p nabi-editor --test lsp_ra -- --ignored`

use nabi_editor::lspclient::LspClient;
use std::time::{Duration, Instant};

/// 초기화→didOpen→진단 수신→정의 이동까지 실제 rust-analyzer로 왕복한다.
#[test]
#[ignore = "rust-analyzer 필요(PATH)"]
fn ra_diagnostics_and_definition() {
    // 미설치 환경은 조용히 통과.
    if std::process::Command::new("rust-analyzer").arg("--version").output().is_err() {
        return;
    }
    // 최소 cargo 프로젝트를 즉석 생성(에러 1개 포함).
    let dir = std::env::temp_dir().join(format!("nabi-lsp-{}", std::process::id()));
    let src_dir = dir.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[package]\nname=\"t\"\nversion=\"0.1.0\"\nedition=\"2021\"\n").unwrap();
    let main = src_dir.join("main.rs");
    let code = "fn helper() -> i32 { 7 }\nfn main() { let x: u8 = helper(); }\n"; // 타입 에러 1개.
    std::fs::write(&main, code).unwrap();

    let mut c = LspClient::start("rust-analyzer", &dir).expect("서버 기동");
    let t0 = Instant::now();
    while !c.ready() && t0.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(c.ready(), "60초 내 initialize 완료");
    c.did_open(&main, code);
    c.did_save(&main); // 플라이체크(cargo check) 진단 트리거.

    // 진단이 올 때까지 폴링(인덱싱 시간 포함 — 최대 120초).
    let t1 = Instant::now();
    let diags = loop {
        let d = c.diagnostics(&main);
        if d.iter().any(|d| d.severity == 1) {
            break d;
        }
        if t1.elapsed() > Duration::from_secs(120) {
            panic!("진단 미수신: {:?}", d);
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    assert!(diags.iter().any(|d| d.line == 1), "에러는 2번째 줄: {diags:?}");

    // helper() 호출 지점(2행 "helper" 위)에서 정의로 이동 → 1행.
    let id = c.request_definition(&main, 1, 26).expect("요청");
    let t2 = Instant::now();
    let def = loop {
        if let Some(d) = c.take_definition(id) {
            break d;
        }
        if t2.elapsed() > Duration::from_secs(30) {
            panic!("정의 응답 없음");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let def = def.expect("정의 위치");
    assert_eq!(def.line, 0, "helper 정의는 1번째 줄");
    assert!(c.alive());
    drop(c);
    let _ = std::fs::remove_dir_all(&dir);
}
