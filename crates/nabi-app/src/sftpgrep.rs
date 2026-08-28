//! **원격 내용 찾기** — 서버 파일 안의 문자열을 찾는다(배치 AD).
//!
//! 로컬 탐색기에는 내용 찾기가 있는데(`findfiles.rs`) 원격에는 이름 찾기(`sftpfind.rs`)만
//! 있었다. 서버에서 "그 설정이 어느 파일에 있더라"를 찾으려면 터미널로 건너가 `grep` 을
//! 쳐야 했다 — 파일 관리자를 쓰던 흐름이 거기서 끊긴다.
//!
//! ## 새로 만들지 않은 것
//!
//! * **일치 규칙**은 로컬의 [`crate::findfiles::grep_lines`] 를 그대로 쓴다. 같은 질문에
//!   두 경로가 다른 답을 내면 사용자는 어느 쪽을 믿어야 할지 알 수 없다.
//! * **파일 목록**은 이름 찾기가 쓰는 트리 훑기를 그대로 쓴다(새 재귀를 만들지 않는다).
//! * **읽기 상한**은 미리보기와 같은 규칙을 따른다(아래).
//!
//! ## 앞부분만 읽는다 — 그리고 그렇다고 말한다
//!
//! 원격 파일은 **크기를 믿을 수 없다**(심볼릭 링크, `/proc` 같은 가짜 파일, 잘못된 stat).
//! 그래서 미리보기와 마찬가지로 **크기를 묻지 않고 처음부터 상한만큼만** 읽는다. 몇 GB짜리를
//! 실수로 끌어올 길을 아예 만들지 않는다.
//!
//! 대신 **못 본 부분이 있으면 결과에 적는다.** 뒤쪽에 있던 일치를 놓친 채 "없음"을 보여 주면
//! 사용자는 서버에 그 문자열이 없다고 믿는다. 그것이 이 기능의 가장 큰 위험이다.

/// 파일 하나에서 읽을 최대 바이트. 미리보기(64KB)보다 넉넉하되 회선을 오래 물지 않는 선.
///
/// 설정 파일·로그 앞부분·소스 코드는 대부분 이 안에 들어온다. 그보다 큰 파일에서 뒤쪽을
/// 찾아야 한다면 그것은 터미널의 `grep` 이 할 일이고, 우리는 그 사실을 숨기지 않는다.
pub(crate) const READ_CAP: usize = 256 * 1024;

/// 훑을 최대 파일 수. 넘으면 거기서 멈추고 그 사실을 결과에 적는다.
pub(crate) const MAX_FILES: usize = 400;

/// 모을 최대 일치 줄 수.
pub(crate) const MAX_HITS: usize = 500;

/// 한 파일에서 찾은 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileHits {
    pub rel: String,
    /// (줄 번호 1부터, 줄 내용)
    pub lines: Vec<(usize, String)>,
    /// 이 파일을 **끝까지 못 봤다**(상한에 걸렸다).
    pub partial: bool,
}

/// 내려받은 앞부분에서 일치를 찾는다. 이진으로 보이면 건너뛴다(`None`).
///
/// 이진 파일을 글자로 훑으면 쓰레기 줄이 결과에 섞인다. 판정은 미리보기와 같은 근거를
/// 쓴다 — NUL 바이트가 있으면 글이 아니다.
pub(crate) fn scan_file(rel: &str, bytes: &[u8], pat: &str, ci: bool, capped: bool) -> Option<FileHits> {
    // 이진 판정과 인코딩 판정 **둘 다** 이미 있는 것을 쓴다. 처음엔 여기서 `contains(&0)` 과
    // `from_utf8_lossy` 를 직접 썼는데, 그러면 미리보기와 판정이 갈라진다 — 같은 파일을
    // 한쪽은 글로 보고 다른 쪽은 이진으로 본다.
    //
    // `from_utf8_lossy` 는 특히 나빴다. **한국 서버에는 CP949 파일이 흔한데** 그것을
    // 대체 문자로 뭉개면 한글 질의는 **영원히 아무것도 못 찾는다.** 못 찾은 이유가 화면에
    // 드러나지도 않는다.
    if nabi_editor::edithex::is_binary(bytes) {
        return None;
    }
    let enc = nabi_editor::editload::detect_encoding(bytes);
    let (text, _, _) = enc.decode(bytes);
    // 마지막 줄은 상한에서 잘렸을 수 있다. 잘린 줄을 결과에 넣으면 없는 내용을 보여 주게 된다.
    let body = if capped { drop_last_line(&text) } else { text.to_string() };
    let lines = crate::findfiles::grep_lines(&body, pat, ci);
    if lines.is_empty() && !capped {
        return None;
    }
    if lines.is_empty() {
        return None;
    }
    Some(FileHits { rel: rel.to_string(), lines, partial: capped })
}

/// 마지막 줄을 버린다 — 상한에서 잘린 조각일 수 있다.
///
/// 줄이 하나뿐이면 버리지 않는다. 아주 긴 한 줄짜리 파일(minified·JSON 한 덩어리)을
/// 통째로 버리면 그 파일은 영원히 안 찾힌다.
fn drop_last_line(s: &str) -> String {
    match s.rfind('\n') {
        Some(i) => s[..i].to_string(),
        None => s.to_string(),
    }
}

/// 대소문자를 무시할 것인가 — 스마트케이스(로컬 내용 찾기와 같은 규칙).
pub(crate) fn smart_case(pat: &str) -> bool {
    nabi_render::smartcase::insensitive(pat)
}

/// 결과를 사람이 읽는 한 덩어리로. 로컬 내용 찾기와 같은 `경로:줄: 내용` 모양이다.
///
/// `stopped` 는 파일 수 상한에 걸려 **다 못 훑었다**는 뜻이다. 이 사실이 빠지면 "없음"이
/// "서버에 없다"로 읽힌다.
pub(crate) fn report(hits: &[FileHits], pat: &str, stopped: bool, scanned: usize) -> (String, usize) {
    let total: usize = hits.iter().map(|f| f.lines.len()).sum();
    let mut out = String::new();
    for f in hits {
        for (n, line) in &f.lines {
            out.push_str(&format!("{}:{}: {}\n", f.rel, n, line));
        }
    }
    let mut notes = Vec::new();
    if hits.iter().any(|f| f.partial) {
        notes.push(format!("일부 파일은 앞 {}KB 까지만 봤습니다", READ_CAP / 1024));
    }
    if stopped {
        notes.push(format!("파일 {scanned}개까지만 훑고 멈췄습니다"));
    }
    if !notes.is_empty() {
        out.push_str(&format!("\n--- {} ---\n", notes.join(" · ")));
    }
    if total == 0 {
        out.insert_str(0, &format!("'{pat}' 를 찾지 못했습니다\n"));
    }
    (out, total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binary_file_is_skipped() {
        // 이진을 글자로 훑으면 쓰레기 줄이 결과에 섞인다.
        let b = b"\x00\x01binary port 22 stuff";
        assert_eq!(scan_file("a.bin", b, "port", true, false), None);
    }

    #[test]
    fn a_match_is_found_with_its_line_number() {
        let f = scan_file("etc/sshd_config", b"a\nPort 2222\nb", "port", true, false).unwrap();
        assert_eq!(f.lines, vec![(2, "Port 2222".to_string())]);
        assert!(!f.partial);
    }

    #[test]
    fn a_truncated_last_line_is_not_reported() {
        // 상한에서 잘린 조각을 결과에 넣으면 없는 내용을 보여 주게 된다.
        let f = scan_file("x", b"first\nPortcut", "portcut", true, true);
        assert!(f.is_none(), "잘렸을 수 있는 마지막 줄은 버린다");
    }

    #[test]
    fn a_single_long_line_is_not_thrown_away() {
        // 줄이 하나뿐인 파일(minified·JSON)까지 버리면 그 파일은 영원히 안 찾힌다.
        let f = scan_file("x.json", b"{\"port\":2222}", "port", true, true).unwrap();
        assert_eq!(f.lines.len(), 1);
    }

    #[test]
    fn smart_case_follows_the_local_rule() {
        assert!(smart_case("port"), "소문자만이면 무시");
        assert!(!smart_case("Port"), "대문자가 있으면 구분");
    }

    #[test]
    fn a_partial_read_is_written_into_the_report() {
        let hits = vec![FileHits { rel: "a".into(), lines: vec![(1, "x".into())], partial: true }];
        let (text, n) = report(&hits, "x", false, 3);
        assert_eq!(n, 1);
        assert!(text.contains("앞 256KB"), "못 본 부분이 있으면 반드시 적는다: {text}");
    }

    #[test]
    fn hitting_the_file_cap_is_written_into_the_report() {
        let (text, _) = report(&[], "x", true, 400);
        assert!(text.contains("400개까지만"), "다 못 훑었다는 사실이 빠지면 '없음'이 '서버에 없다'로 읽힌다");
    }

    #[test]
    fn finding_nothing_says_so_instead_of_showing_an_empty_page() {
        let (text, n) = report(&[], "needle", false, 10);
        assert_eq!(n, 0);
        assert!(text.contains("needle"), "무엇을 못 찾았는지 말한다");
    }

    #[test]
    fn the_report_uses_the_same_shape_as_the_local_search() {
        let hits = vec![FileHits { rel: "etc/x.conf".into(), lines: vec![(7, "Port 22".into())], partial: false }];
        let (text, _) = report(&hits, "port", false, 1);
        assert!(text.starts_with("etc/x.conf:7: Port 22"), "경로:줄: 내용 — 로컬과 같은 모양: {text}");
    }
    #[test]
    fn a_korean_cp949_file_is_searchable() {
        // 이 시험이 이 배치에서 가장 중요하다. 한국 서버에는 CP949 파일이 흔한데,
        // `from_utf8_lossy` 로 읽으면 한글이 대체 문자로 뭉개져 **한글 질의는 영원히
        // 아무것도 못 찾는다.** 못 찾은 이유가 화면에 드러나지도 않는다.
        let (bytes, _, _) = encoding_rs::EUC_KR.encode("첫 줄
포트 설정은 2222 입니다
끝");
        let f = scan_file("etc/설정.conf", &bytes, "포트", true, false)
            .expect("CP949 한글 파일에서 한글을 찾아야 한다");
        assert_eq!(f.lines.len(), 1);
        assert!(f.lines[0].1.contains("포트"), "{:?}", f.lines);
    }

    #[test]
    fn a_utf8_korean_file_is_searchable_too() {
        let f = scan_file("x", "설정
포트 2222".as_bytes(), "포트", true, false).unwrap();
        assert_eq!(f.lines.len(), 1);
    }

    #[test]
    fn binary_detection_matches_the_preview_window() {
        // 판정이 갈라지면 같은 파일을 한쪽은 글로, 다른 쪽은 이진으로 본다.
        let b = b" binary";
        assert!(nabi_editor::edithex::is_binary(b));
        assert_eq!(scan_file("a.bin", b, "binary", true, false), None);
    }

}
