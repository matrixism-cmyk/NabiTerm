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

    // 심볼 정보(hover): helper 호출 위 — 시그니처가 와야 한다.
    let id = c.request_hover(&main, 1, 26).expect("hover 요청");
    let t3 = Instant::now();
    let info = loop {
        if let Some(h) = c.take_hover(id) {
            break h;
        }
        assert!(t3.elapsed() < Duration::from_secs(30), "hover 응답 없음");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(info.unwrap_or_default().contains("helper"), "hover에 심볼명 포함");

    // 참조 찾기: helper 정의 위 — 선언+호출 2곳.
    let id = c.request_references(&main, 0, 4).expect("references 요청");
    let t4 = Instant::now();
    let refs = loop {
        if let Some(r) = c.take_references(id) {
            break r;
        }
        assert!(t4.elapsed() < Duration::from_secs(30), "references 응답 없음");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(refs.len() >= 2, "선언+호출 최소 2곳: {refs:?}");

    // 이름 바꾸기: helper → helper2. WorkspaceEdit를 순수 적용해 결과를 검증한다.
    let id = c.request_rename(&main, 0, 4, "helper2").expect("rename 요청");
    let t5 = Instant::now();
    let files = loop {
        if let Some(f) = c.take_rename(id) {
            break f;
        }
        assert!(t5.elapsed() < Duration::from_secs(30), "rename 응답 없음");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(!files.is_empty(), "rename 편집 목록");
    let fe = files.iter().find(|f| f.path.ends_with("main.rs")).expect("main.rs 편집");
    let renamed = nabi_editor::lspread::apply_edits(code, &fe.edits);
    assert!(renamed.contains("fn helper2()"), "선언 변경: {renamed}");
    assert!(renamed.contains("helper2();") || renamed.contains("= helper2()"), "호출 변경: {renamed}");

    // 문서 포맷팅: 일부러 찌그러뜨린 텍스트를 동기화 후 rustfmt 결과를 적용해 확인.
    let ugly = "fn helper()->i32{7}\nfn main(){let _x:i32=helper();}\n";
    c.did_change(&main, ugly);
    let id = c.request_formatting(&main, 4).expect("format 요청");
    let t6 = Instant::now();
    let edits = loop {
        if let Some(e) = c.take_formatting(id) {
            break e;
        }
        assert!(t6.elapsed() < Duration::from_secs(30), "format 응답 없음");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(!edits.is_empty(), "포맷 편집이 와야 함");
    let pretty = nabi_editor::lspread::apply_edits(ugly, &edits);
    assert!(pretty.contains("fn helper() -> i32"), "rustfmt 결과: {pretty}");

    // 자동완성: main 안에서 "hel" 접두어 → helper 후보가 와야 한다.
    let comp_src = "fn helper() -> i32 { 7 }\nfn main() { let _x = hel; }\n";
    c.did_change(&main, comp_src);
    let id = c.request_completion(&main, 1, 24).expect("completion 요청"); // "hel" 끝.
    let t7 = Instant::now();
    let items = loop {
        if let Some(v) = c.take_completion(id) {
            break v;
        }
        assert!(t7.elapsed() < Duration::from_secs(30), "completion 응답 없음");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(items.iter().any(|i| i.label.starts_with("helper")), "helper 후보: {:?}", items.iter().map(|i| &i.label).take(8).collect::<Vec<_>>());
    assert!(c.alive());
    drop(c);
    let _ = std::fs::remove_dir_all(&dir);
}
