//! **일괄 이름변경 규칙** — 로컬 창과 원격 창이 같은 규칙을 쓴다(배치 AJ).
//!
//! 원래 이 규칙은 SFTP 쪽(`sftpentries`)에만 있었고 로컬 탐색기에는 일괄 이름변경이
//! **아예 없었다.** Total Commander 계열이 이 기능으로 자리를 잡은 것을 생각하면 큰 구멍이다.
//!
//! 로컬에 같은 기능을 만들면서 **규칙을 복사하지 않고 여기로 옮겼다.** 정렬 규칙을
//! `browsersort::entry_cmp` 한 곳에 둔 것과 같은 이유다 — 같은 폴더를 두 창이 다르게
//! 보여 주면 사용자는 어느 쪽을 믿어야 할지 모른다.

/// 일괄 이름변경: name에서 find→replace 치환한 새 이름(바뀌지 않으면 None).
/// replace 토큰: `{n}`=순번, `{nn}`/`{nnn}`=0채움 순번, `{name}`=확장자 뺀 원본명, `{ext}`=확장자.
pub fn batch_new_name(name: &str, find: &str, replace: &str, idx: usize) -> Option<String> {
    if find.is_empty() || !name.contains(find) {
        return None;
    }
    // 확장자 분리(숨김파일 .bashrc는 통째로 name 취급).
    let (base, ext) = match name.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => (b, e),
        _ => (name, ""),
    };
    let replace = replace
        .replace("{nnn}", &format!("{idx:03}"))
        .replace("{nn}", &format!("{idx:02}"))
        .replace("{n}", &idx.to_string())
        .replace("{name}", base)
        .replace("{ext}", ext);
    let new = name.replace(find, &replace);
    (new != name && !new.is_empty()).then_some(new)
}


/// 한 폴더 안에서 **한꺼번에** 바꿀 이름들을 계획한다(배치 AJ).
///
/// 실제로 파일을 건드리기 전에 **전부 미리 계산해 검사한다.** 도중에 멈추면 절반만 바뀐
/// 폴더가 남고, 그 상태에서 되돌리려면 무엇이 바뀌었는지 사용자가 기억해야 한다.
///
/// 막는 것 둘:
/// * **서로 같은 새 이름** — 둘을 같은 이름으로 바꾸면 하나가 다른 하나를 덮어쓴다.
/// * **이미 있는 이름과 충돌** — 바꾸지 않는 파일 위에 덮어쓴다.
///
/// 둘 다 조용히 파일을 잃는 길이라 **계획 단계에서 통째로 거절한다.** 하나만 빼고 하면
/// 사용자는 무엇이 빠졌는지 모른 채 "됐다"고 믿는다.
/// **순번 `{n}` 은 실제로 바뀌는 파일에만 붙는다.**
///
/// 예전에는 목록에서의 자리(`i + 1`)를 그대로 썼다. 그래서 스무 개 중 셋만 규칙에 걸리면
/// 번호가 `1, 7, 15` 처럼 **구멍 난 채로** 나왔다. 원격(SFTP) 쪽은 처음부터 바뀌는 것만
/// 세고 있어서, 같은 규칙을 같은 파일에 걸어도 두 창이 다른 이름을 내놓았다.
/// 사람이 기대하는 것은 `1, 2, 3` 이므로 그쪽으로 맞춘다.
///
/// `lang` 은 거절 사유를 사람 말로 적기 위한 것이다 — 예전에는 한국어가 박혀 있어
/// 영어·일본어로 쓰는 사람에게도 한국어가 나왔다.
pub fn plan_batch(
    names: &[String],
    find: &str,
    replace: &str,
    lang: nabi_i18n::Lang,
) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut n = 1usize;
    for name in names {
        if let Some(new) = batch_new_name(name, find, replace, n) {
            out.push((name.clone(), new));
            n += 1;
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for (from, to) in &out {
        if seen.contains(&to.as_str()) {
            return Err(format!("{to} \u{2190} {}", nabi_i18n::tr(lang, "rename.dup")));
        }
        // 바꾸지 않는 파일과 부딪히는가(바뀌는 것끼리는 위에서 이미 봤다).
        if names.iter().any(|n| n == to) && !out.iter().any(|(f, _)| f == to) {
            return Err(format!("{to} \u{2190} {}", nabi_i18n::tr(lang, "rename.exists")));
        }
        let _ = from;
        seen.push(to);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::batch_new_name;

    #[test]
    fn batch_rename_replaces() {
        assert_eq!(batch_new_name("a.txt", ".txt", ".bak", 1).as_deref(), Some("a.bak"));
        assert_eq!(batch_new_name("b.log", ".txt", ".bak", 1), None);
        assert_eq!(batch_new_name("x", "", "y", 1), None);
        assert_eq!(batch_new_name("img1", "img", "photo", 1).as_deref(), Some("photo1"));
        // {n} 순번 치환.
        assert_eq!(batch_new_name("draft", "draft", "v{n}", 3).as_deref(), Some("v3"));
        assert_eq!(batch_new_name("a_x", "x", "{n}", 7).as_deref(), Some("a_7"));
        // 0채움 순번 {nn}/{nnn} + {name}/{ext} 토큰(find는 한 번만 나오는 부분 사용).
        assert_eq!(batch_new_name("photo.png", "photo", "{nnn}", 5).as_deref(), Some("005.png"));
        assert_eq!(batch_new_name("photo.png", "photo", "{name}_{nn}", 5).as_deref(), Some("photo_05.png"));
        assert_eq!(batch_new_name("photo.png", "photo", "{name}-{ext}", 1).as_deref(), Some("photo-png.png"));
    }

    use super::plan_batch;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_plain_batch_is_planned_in_order() {
        let p = plan_batch(&v(&["a.txt", "b.txt"]), ".txt", ".bak", nabi_i18n::Lang::Ko).unwrap();
        assert_eq!(p, vec![("a.txt".into(), "a.bak".into()), ("b.txt".into(), "b.bak".into())]);
    }

    #[test]
    fn files_that_do_not_match_are_left_alone() {
        let p = plan_batch(&v(&["a.txt", "keep.log"]), ".txt", ".bak", nabi_i18n::Lang::Ko).unwrap();
        assert_eq!(p.len(), 1, "안 걸린 파일은 계획에 없다");
    }

    #[test]
    fn two_files_becoming_one_name_is_refused() {
        // 하나가 다른 하나를 덮어쓴다 — 조용히 파일을 잃는 길이다.
        //
        // 시험을 세 번 고쳤다. 처음엔 둘째가 아예 안 걸렸고, 다음엔 둘이 서로 다른 이름이
        // 됐고, 빈 질의는 규칙이 애초에 거절한다. **글자를 지워 길이가 같아질 때**만 둘이
        // 같은 곳으로 모인다 — 짐작을 멈추고 규칙을 손으로 돌려 본 뒤에 알았다.
        let e = plan_batch(&v(&["aa.txt", "aaa.txt"]), "a", "", nabi_i18n::Lang::Ko).unwrap_err();
        assert!(e.contains("같은 이름"), "{e}");
    }

    #[test]
    fn colliding_with_a_file_we_are_not_renaming_is_refused() {
        let e = plan_batch(&v(&["a.txt", "a.bak"]), ".txt", ".bak", nabi_i18n::Lang::Ko).unwrap_err();
        assert!(e.contains("이미 있는"), "{e}");
    }

    #[test]
    fn a_swap_is_allowed_because_both_names_move() {
        // a.txt→a.bak 이면서 a.bak 도 함께 바뀌면 겹치지 않는다. 통째로 거절하면
        // 정상적인 일괄 변경까지 막는다.
        let p = plan_batch(&v(&["a.bak", "a.txt"]), ".txt", ".bak2", nabi_i18n::Lang::Ko);
        assert!(p.is_ok(), "{p:?}");
    }

    #[test]
    fn the_numbering_counts_from_one() {
        let p = plan_batch(&v(&["x.txt", "y.txt"]), ".txt", "-{n}.txt", nabi_i18n::Lang::Ko).unwrap();
        assert_eq!(p[0].1, "x-1.txt");
        assert_eq!(p[1].1, "y-2.txt");
    }

    /// **번호에 구멍이 나면 안 된다.**
    ///
    /// 예전에는 목록에서의 자리를 그대로 썼다. 그래서 규칙에 안 걸리는 파일이 사이에 있으면
    /// `1, 3` 처럼 건너뛴 번호가 나왔다. 원격(SFTP) 쪽은 바뀌는 것만 세고 있어서, 같은 규칙을
    /// 같은 파일에 걸어도 두 창이 다른 이름을 내놓았다.
    #[test]
    fn skipped_files_do_not_eat_a_number() {
        // 가운데 `skip.log` 는 규칙에 안 걸린다 — 번호를 먹으면 안 된다.
        let names = v(&["a.txt", "skip.log", "b.txt"]);
        let p = plan_batch(&names, ".txt", "-{n}.txt", nabi_i18n::Lang::Ko).unwrap();
        assert_eq!(p.len(), 2, "걸리는 것은 둘뿐이다");
        assert_eq!(p[0].1, "a-1.txt");
        assert_eq!(p[1].1, "b-2.txt", "예전에는 여기가 b-3.txt 였다");
    }
}
