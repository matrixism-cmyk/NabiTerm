//! castplain 시험 — 실제 기록에서 겪은 것들을 그대로 옮겨 놓았다.

use super::*;

const E: char = '\u{1b}';

fn esc(s: &str) -> String {
    s.replace('~', &E.to_string())
}

#[test]
fn 색은_버리고_글자만_남긴다() {
    assert_eq!(flatten(&esc("~[38;2;1;2;3m빨강~[m")), "빨강\n");
}

/// 이것이 이 모듈을 다시 쓴 이유다.
///
/// 그냥 걷어내기만 했더니 "도움말은이미두툼합니다" 가 나왔다. 그 프로그램은 빈칸을
/// 공백으로 찍지 않고 커서를 옮겨 만든다 — 옮긴 만큼을 공백으로 되살려야 한다.
#[test]
fn 커서를_옮긴_만큼은_공백이_된다() {
    // 오른쪽으로 3칸.
    assert_eq!(flatten(&esc("가~[3C나")), "가   나\n");
    // 10번째 열로.
    assert_eq!(flatten(&esc("가~[10G나")), "가        나\n");
}

#[test]
fn 절대_위치는_줄을_끊고_열을_맞춘다() {
    // 3열로 갔으니 앞에 공백 두 칸.
    assert_eq!(flatten(&esc("첫줄~[5;3H둘째")), "첫줄\n  둘째\n");
}

#[test]
fn 같은_줄을_다시_그리면_한_번만_남는다() {
    // 화면을 세 번 다시 그린 흔적 — 읽을 때는 한 줄이면 된다.
    let s = esc("~[1;1H같은 줄~[1;1H같은 줄~[1;1H같은 줄");
    assert_eq!(flatten(&s), "같은 줄\n");
}

#[test]
fn 다른_줄이면_남긴다() {
    let s = esc("~[1;1H첫~[2;1H둘~[3;1H첫");
    assert_eq!(flatten(&s), "첫\n둘\n첫\n");
}

#[test]
fn 커서_뒤_지우기는_뒤를_잘라낸다() {
    // 긴 글을 쓰고 3열로 돌아가 뒤를 지운다.
    assert_eq!(flatten(&esc("가나다라마~[3G~[K")), "가나\n");
}

#[test]
fn 제목같은_것도_버린다() {
    assert_eq!(flatten(&esc("~]0;제목\u{7}본문")), "본문\n");
    // 끝맺음이 ESC \ 인 형태(문자열 안에서는 백슬래시가 둘이다).
    assert_eq!(flatten(&esc("~]7771;a;{}~\\본문")), "본문\n");
}

#[test]
fn 빈줄이_길게_이어지면_줄인다() {
    assert_eq!(flatten("가\n\n\n\n\n나"), "가\n\n\n나\n");
}

#[test]
fn 캐리지리턴은_줄머리로_돌아간다() {
    // 같은 줄을 덮어쓴다 — 뒤에 남은 글자는 그대로 있다(터미널과 같다).
    assert_eq!(flatten("abcde\rxy"), "xycde\n");
}

#[test]
fn 기록에서_글만_뽑는다() {
    let cast = concat!(
        "{\"version\":2,\"width\":80,\"height\":24}\n",
        "[0.1, \"o\", \"첫 줄\\r\\n\"]\n",
        "[0.2, \"o\", \"둘째 줄\\r\\n\"]\n",
        "[0.3, \"i\", \"이건 입력이라 빼야 한다\"]\n",
    );
    assert_eq!(cast_to_plain(cast), "첫 줄\n둘째 줄\n");
}

/// **진짜 기록으로 해 본다.** 합성 데이터로만 시험하면 통과한다는 것만 증명한다.
///
/// ```text
/// NABI_CAST=<기록.log> cargo test -p nabi-app real_cast -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn real_cast() {
    let Ok(path) = std::env::var("NABI_CAST") else {
        panic!("NABI_CAST 에 기록 파일 경로를 넣고 부를 것");
    };
    let text = read_log(std::path::Path::new(&path)).expect("기록을 읽지 못했다");
    let plain = cast_to_plain(&text);
    let lines: Vec<&str> = plain.lines().collect();
    println!("원본 {} 글자 → 편 글 {} 글자 · {} 줄", text.len(), plain.len(), lines.len());
    let mid = lines.len() / 2;
    println!("--- 가운데 25줄 ---");
    for l in lines.iter().skip(mid).take(25) {
        println!("{}", l.chars().take(150).collect::<String>());
    }
    assert!(lines.len() > 100, "펴 낸 줄이 너무 적다 — 무언가 잘못됐다");
    // 표식이 박힌 기록이면 **정확도를 숫자로** 잰다(scratchpad/precise.ps1 가 만든다).
    // KEEP 은 하나도 빠지면 안 되고, CHROME 은 하나도 남으면 안 된다.
    let keep = lines.iter().filter(|l| l.contains("KEEP-")).count();
    let chrome = lines.iter().filter(|l| l.contains("CHROME-")).count();
    if keep > 0 || chrome > 0 {
        println!("정확도: KEEP {keep} 줄 살아남음 · CHROME {chrome} 줄 남음");
    }
}

#[test]
fn 기호만_남은_줄은_버린다() {
    // 커서를 옮겨 그린 스피너 머리 — 버린다.
    assert_eq!(flatten(&esc("~[1;1H●~[2;1H내용")), "내용\n");
    // 흘러온 줄이면 사람이 찍은 것일 수 있으니 남긴다.
    assert_eq!(flatten("●\n내용"), "●\n내용\n");
}
