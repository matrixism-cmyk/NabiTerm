//! 소스가 부르는 **모든 i18n 키가 실제로 번역되는지** 검사한다.
//!
//! `tr()`는 없는 키를 만나면 `"?"`를 돌려준다 — 화면에 물음표가 박히는데 컴파일도 되고
//! clippy도 조용하다. 실제로 그렇게 새어 나갈 뻔했다(2026-08-25, "새로워진 점" 창의
//! 닫기 단추가 `qc.close`라는 없는 키를 불렀다).
//!
//! 카탈로그 쪽 시험(`no_missing_translations`)은 **등록된 키에 세 언어가 다 있는지**만 본다.
//! 등록되지 않은 키를 부르는 것은 반대 방향의 결함이라 그 시험으로는 잡히지 않는다.
//!
//! 카탈로그를 파싱하지 않고 `tr()`에 직접 물어본다 — 카탈로그 항목이 여러 줄에 걸쳐 있는
//! 경우가 있어서, 파싱하려던 첫 시도는 멀쩡한 키 50개를 없다고 잘못 보고했다.

use nabi_i18n::{tr, Lang};
use std::collections::BTreeMap;

/// 워크스페이스 루트(이 크레이트의 두 단계 위).
///
/// `..`를 그대로 두면 경로 조각에 `nabi-i18n`이 남아 아래 제외 검사가 **모든 파일**을
/// 걸러 버린다(그래서 처음엔 0개를 읽었다). 실제 경로로 펴 둔다.
fn workspace_root() -> std::path::PathBuf {
    let raw = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    raw.canonicalize().unwrap_or(raw)
}

/// 소스에서 `tr(…, "key")` 꼴을 모은다(키 → 처음 본 파일 이름).
fn used_keys(root: &std::path::Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target") {
                    continue; // 빌드 산출물은 우리 소스가 아니다.
                }
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            if p.components().any(|c| c.as_os_str() == "nabi-i18n") {
                continue; // 카탈로그 자신은 제외.
            }
            let Ok(text) = std::fs::read_to_string(&p) else { continue };
            let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
            for k in scan_tr_calls(&text) {
                out.entry(k).or_insert_with(|| name.clone());
            }
        }
    }
    out
}

/// `tr(<무엇이든>, "key")` 에서 키만 뽑는다. 호출이 한 줄에 있다고 본다.
fn scan_tr_calls(text: &str) -> Vec<String> {
    let (bytes, mut out, mut i) = (text.as_bytes(), Vec::new(), 0usize);
    while let Some(rel) = text[i..].find("tr(") {
        let at = i + rel;
        i = at + 3;
        // 앞 글자가 식별자면 `str(`·`attr(` 같은 다른 이름이다.
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        let Some(end) = text[i..].find(')') else { break };
        let args = &text[i..i + end];
        if args.contains('\n') {
            continue;
        }
        let mut parts = args.split('"');
        let _ = parts.next(); // 첫 인자(lang)는 문자열이 아니다.
        if let Some(k) = parts.next() {
            if is_key(k) {
                out.push(k.to_string());
            }
        }
    }
    out
}

/// 키처럼 생겼는가 — 소문자·숫자·점·밑줄만, 점이 하나는 있어야 한다.
fn is_key(s: &str) -> bool {
    !s.is_empty()
        && s.contains('.')
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
}

/// 소스가 부르는 키는 전부 번역이 있어야 한다.
#[test]
fn every_key_used_in_the_source_is_translated() {
    let used = used_keys(&workspace_root());
    assert!(used.len() > 200, "소스를 제대로 읽지 못했다({}개)", used.len());
    let missing: Vec<String> = used
        .iter()
        .filter(|(k, _)| tr(Lang::Ko, k) == "?")
        .map(|(k, f)| format!("{k}  ({f})"))
        .collect();
    assert!(
        missing.is_empty(),
        "번역이 없는 키를 부른다 — 화면에 물음표가 나온다:\n{}",
        missing.join("\n")
    );
}

/// 세 언어 모두에 있어야 한다 — 한국어만 넣고 영어를 빠뜨리는 실수를 잡는다.
#[test]
fn used_keys_are_translated_in_all_three_languages() {
    let used = used_keys(&workspace_root());
    let mut bad = Vec::new();
    for (k, f) in &used {
        for (lang, name) in [(Lang::En, "en"), (Lang::Ko, "ko"), (Lang::Ja, "ja")] {
            if tr(lang, k) == "?" {
                bad.push(format!("{k} [{name}]  ({f})"));
            }
        }
    }
    assert!(bad.is_empty(), "언어가 빠진 키:\n{}", bad.join("\n"));
}
