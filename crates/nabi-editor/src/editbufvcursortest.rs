//! `editbufvcursor` 시험 — 위/아래 커서 늘리기.

use crate::editbuf::EditBuf;

fn buf(text: &str) -> EditBuf {
    EditBuf::new_buf(text, "UTF-8".into(), "\n")
}

#[test]
fn a_cursor_is_added_on_the_line_below() {
    let mut b = buf("aaa\nbbb\nccc");
    b.set_cursor(1); // 첫 줄 1열
    assert!(b.add_cursor_vertical(1));
    assert_eq!(b.sel.len(), 2);
    let cols: Vec<usize> = b.sel.ranges().iter().map(|r| r.head).collect();
    assert!(cols.contains(&1) && cols.contains(&5), "{cols:?}");
}

#[test]
fn it_also_goes_up() {
    let mut b = buf("aaa\nbbb\nccc");
    b.set_cursor(5); // 둘째 줄
    assert!(b.add_cursor_vertical(-1));
    assert_eq!(b.sel.len(), 2);
}

/// **누를 때마다 한 줄씩 뻗어야 한다** — 주 커서에서만 재면 두 번째부터 제자리걸음이 된다.
#[test]
fn pressing_again_extends_further() {
    let mut b = buf("aaa\nbbb\nccc\nddd");
    b.set_cursor(0);
    assert!(b.add_cursor_vertical(1));
    assert!(b.add_cursor_vertical(1));
    assert_eq!(b.sel.len(), 3, "두 번 눌렀는데 두 줄만 잡혔다");
    assert!(b.add_cursor_vertical(1));
    assert_eq!(b.sel.len(), 4);
}

/// 문서 끝/처음을 넘어가지 않는다.
#[test]
fn it_stops_at_the_edges() {
    let mut b = buf("only one line");
    b.set_cursor(2);
    assert!(!b.add_cursor_vertical(1));
    assert!(!b.add_cursor_vertical(-1));
    assert_eq!(b.sel.len(), 1);
}

/// **짧은 줄에서는 줄 끝에 놓는다** — 그래야 타자를 쳤을 때 모든 줄에 들어간다.
#[test]
fn a_short_line_gets_its_end_not_a_clamped_middle() {
    let mut b = buf("aaaaaaaa\nbb");
    b.set_cursor(6); // 첫 줄 6열
    assert!(b.add_cursor_vertical(1));
    let heads: Vec<usize> = b.sel.ranges().iter().map(|r| r.head).collect();
    // 둘째 줄은 9(줄 시작) + 2 = 11이 끝이다.
    assert!(heads.contains(&11), "짧은 줄 끝에 놓이지 않았다: {heads:?}");
}

/// 탭이 있으면 글자 수가 아니라 **화면 열**을 맞춰야 세로로 가지런하다.
#[test]
fn the_display_column_is_kept_across_tabs() {
    let mut b = buf("\tX\nabcdX");
    // 첫 줄: 탭(0→4) 다음 X는 화면 4열. 문자 위치는 1.
    b.set_cursor(1);
    assert!(b.add_cursor_vertical(1));
    let heads: Vec<usize> = b.sel.ranges().iter().map(|r| r.head).collect();
    // 둘째 줄 시작은 3, 화면 4열은 "abcd" 다음 = 3 + 4 = 7.
    assert!(heads.contains(&7), "탭을 글자 하나로 셌다: {heads:?}");
}

/// 같은 줄에 또 놓으면 병합돼 개수가 그대로다 — 그럴 바엔 false를 낸다.
#[test]
fn it_refuses_to_add_where_a_cursor_already_is() {
    let mut b = buf("aaa\nbbb");
    b.set_cursor(0);
    assert!(b.add_cursor_vertical(1));
    let n = b.sel.len();
    assert!(!b.add_cursor_vertical(-1), "이미 커서가 있는 줄에 또 놓았다");
    assert_eq!(b.sel.len(), n);
}

/// 여러 커서로 친 글자가 모든 줄에 들어가야 이 기능이 뜻이 있다.
#[test]
fn typing_reaches_every_added_cursor() {
    let mut b = buf("aaa\nbbb\nccc");
    b.set_cursor(0);
    let _ = b.add_cursor_vertical(1);
    let _ = b.add_cursor_vertical(1);
    b.insert_multi("X");
    assert_eq!(b.rope.to_string(), "Xaaa\nXbbb\nXccc");
}

#[test]
fn an_empty_document_is_harmless() {
    let mut b = buf("");
    assert!(!b.add_cursor_vertical(1));
    assert!(!b.add_cursor_vertical(-1));
}
