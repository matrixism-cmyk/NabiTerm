//! 세션 기록(`.cast`)을 **사람이 읽는 글**로 펴 낸다.
//!
//! ## 왜 필요한가
//!
//! 클로드 코드 같은 프로그램은 화면을 위에서 아래로 흘려보내지 않는다. 커서를 특정 줄로
//! 옮겨 그 자리를 **덮어 그린다.** 그래서 터미널 스크롤백에는 거의 아무것도 안 쌓인다 —
//! 6시간을 일해도 450줄만 남았다(2026-08-29 실측). 휠을 올려도 볼 것이 없다.
//!
//! 사라진 것이 아니다. 바이트 기록에는 다 있다(같은 세션 9MB, 사건 115,349개). 다만 그
//! 기록은 제어 신호가 섞인 형식이라 그대로 열면 읽을 수 없다.
//!
//! ## 왜 그냥 걷어내기만 하면 안 되는가
//!
//! 처음에는 제어 신호를 지우고 글자만 남겼다. 실제 기록으로 해 보니 글은 나왔는데
//! **띄어쓰기가 전부 사라졌다** — "도움말은이미두툼합니다" 처럼. 그 프로그램이 빈칸을
//! 공백 문자로 찍지 않고 **커서를 그만큼 옮겨** 만들기 때문이다.
//!
//! 그래서 지우기만 하지 않고, 커서를 옮기는 신호는 **공백으로 되살린다.** 화면을 다시
//! 그리는 흉내를 조금만 내는 셈이다.
//!
//! ## 무엇을 하는가
//!
//! * `ESC[<행>;<열>H` — 줄을 끊고 그 열까지 공백을 채운다.
//! * `ESC[<n>C` / `ESC[<n>G` — 그만큼 오른쪽으로(또는 그 열로) 옮긴다 = 공백.
//! * `ESC[K` — 커서 뒤를 지운다.
//! * 색·커서 숨김 같은 나머지 신호와 `\r` 은 버린다.
//! * **바로 앞과 똑같은 줄은 버린다** — 같은 화면을 수십 번 다시 그린 흔적이다.
//!
//! ## 스피너 같은 화면 장식을 가려내는 법
//!
//! 그렇게 펴 냈더니 148,398줄이 나왔는데 대부분이 "빙글빙글 도는 표시"였다. 한 틱마다
//! 모양이 조금씩 달라서 "앞줄과 같으면 버린다"로는 걸러지지 않는다.
//!
//! 프로그램 이름이나 문구를 박아 두고 거르지 않는다 — 다음 프로그램에서 또 어긋난다.
//! 대신 **숫자로 가린다.** 먼저 한 번 훑어 *어느 행에 몇 번 그렸는지* 센다. 어떤 행이
//! 다른 행들보다 압도적으로 자주 그려졌다면 그 행은 내용이 아니라 장식이다.
//!
//! 실측이 그것을 뚜렷하게 보여 줬다 — 36행 화면에서 34행 52,973번, 30행 39,469번인데
//! 보통 행은 1,000번 남짓이었다. 서른 배 차이다.
//!
//! 흘러가며 찍힌 줄(줄바꿈으로 내려온 줄)은 이 규칙을 적용하지 않는다. 일반 셸의 출력은
//! 커서를 옮겨 찍지 않으므로 하나도 잃지 않는다.

/// 한 줄씩 쌓아 가며 펴 내는 상태.
struct Flat {
    out: Vec<String>,
    line: String,
    col: usize,
    /// 지금 쓰고 있는 줄이 몇 행에 그려지는 중인가(커서를 옮겨 갔다면).
    row: usize,
    /// 이 줄이 **커서를 옮겨** 시작됐는가. 흘러온 줄이면 false.
    positioned: bool,
    /// 장식으로 판정된 행들 — 여기 그려진 줄은 버린다.
    chrome: std::collections::HashSet<usize>,
}

impl Flat {
    fn new(chrome: std::collections::HashSet<usize>) -> Self {
        Self {
            out: Vec::new(),
            line: String::new(),
            col: 0,
            row: 0,
            positioned: false,
            chrome,
        }
    }

    /// 지금 열까지 공백을 채운다 — 커서만 옮기고 쓴 글자를 제자리에 놓기 위해서다.
    fn pad(&mut self) {
        let have = self.line.chars().count();
        for _ in have..self.col {
            self.line.push(' ');
        }
    }

    fn put(&mut self, c: char) {
        self.pad();
        // 이미 그 자리에 글자가 있으면 덮어쓴다(다시 그리기).
        if self.line.chars().count() > self.col {
            let mut n: String = self.line.chars().take(self.col).collect();
            n.push(c);
            n.extend(self.line.chars().skip(self.col + 1));
            self.line = n;
        } else {
            self.line.push(c);
        }
        self.col += 1;
    }

    /// 줄을 끝낸다. 앞줄과 같으면 버린다 — 같은 화면을 다시 그린 것이다.
    fn newline(&mut self) {
        let done = self.line.trim_end().to_string();
        let drop = self.positioned
            && (self.chrome.contains(&self.row) || only_symbols(&done));
        self.line.clear();
        self.col = 0;
        self.positioned = false;
        if drop {
            return; // 화면 장식이다 — 읽을 글이 아니다.
        }
        if !done.trim().is_empty() && self.out.last().is_some_and(|p| *p == done) {
            return;
        }
        // 빈 줄이 셋 넘게 이어지면 버린다. 맨 앞의 빈 줄도 버린다 — 프로그램이 첫
        // 화면을 그리기 전에 커서부터 옮기는 일이 흔해서, 안 버리면 늘 빈 줄로 시작한다.
        if done.trim().is_empty() {
            if self.out.is_empty() {
                return;
            }
            let tail = self.out.iter().rev().take(2).filter(|l| l.trim().is_empty()).count();
            if tail >= 2 {
                return;
            }
        }
        self.out.push(done);
    }

    fn finish(mut self) -> String {
        if !self.line.trim().is_empty() {
            self.newline();
        }
        self.out.join("\n") + "\n"
    }
}

/// 글자라고는 기호 한두 개뿐인 줄인가.
///
/// 스피너의 머리 글자(`\u{25cf}` `\u{2736}` 같은 것)만 남은 줄이 그렇다. 글이 아니다.
/// 커서를 옮겨 그린 줄에만 적용한다 — 흘러온 줄은 사람이 찍은 것일 수 있다.
fn only_symbols(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > 2 {
        return false;
    }
    !t.chars().any(|c| c.is_alphanumeric())
}

/// CSI 하나를 읽어 매개변수와 마지막 글자를 돌려준다.
fn read_csi(it: &mut std::iter::Peekable<std::str::Chars>) -> (Vec<usize>, char) {
    let mut params = String::new();
    let mut last = '\0';
    for c in it.by_ref() {
        if ('\u{40}'..='\u{7e}').contains(&c) {
            last = c;
            break;
        }
        params.push(c);
    }
    let nums = params
        .trim_start_matches('?')
        .split(';')
        .map(|p| p.trim().parse::<usize>().unwrap_or(0))
        .collect();
    (nums, last)
}

/// OSC 를 끝까지 읽어 버린다(BEL 또는 `ESC \` 로 끝난다).
fn skip_osc(it: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(c) = it.next() {
        if c == '\u{7}' {
            return;
        }
        if c == '\u{1b}' && it.peek() == Some(&'\\') {
            it.next();
            return;
        }
    }
}

/// 기록의 바이트 흐름을 읽을 수 있는 글로 편다.
pub(crate) fn flatten(s: &str) -> String {
    let mut f = Flat::new(chrome_rows(s));
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' => f.newline(),
            '\r' => f.col = 0,
            '\t' => f.col = (f.col / 8 + 1) * 8,
            '\u{1b}' => match it.next() {
                Some('[') => {
                    let (p, last) = read_csi(&mut it);
                    let n = |i: usize| p.get(i).copied().unwrap_or(0);
                    match last {
                        // 커서를 그 자리로 — 줄을 끊고 그 열까지 공백을 채운다.
                        'H' | 'f' => {
                            f.newline();
                            f.row = n(0).saturating_sub(1);
                            f.col = n(1).saturating_sub(1);
                            f.positioned = true;
                        }
                        'C' => f.col += n(0).max(1),           // 오른쪽으로
                        'G' | '`' => f.col = n(0).saturating_sub(1), // 그 열로
                        // 커서 뒤를 지운다 — 뒤에 남은 글자를 잘라낸다.
                        'K' if n(0) == 0 => {
                            f.line = f.line.chars().take(f.col).collect();
                        }
                        _ => {}
                    }
                }
                Some(']') => skip_osc(&mut it),
                _ => {}
            },
            c if (c as u32) < 0x20 => {} // 나머지 제어 문자는 버린다.
            c => f.put(c),
        }
    }
    f.finish()
}

/// 유난히 자주 덮어 그려진 행들 — 스피너·상태줄 같은 화면 장식이다.
///
/// ## 자를 두 개 쓴다 (2026-08-30, 실측으로 고침)
///
/// **① 다른 행보다 유난히 자주 그려졌나** — 중앙값의 여덟 배. 실측에서 장식과 보통 줄의
/// 차이가 서른 배였으니 여덟 배는 넉넉하다. 어중간하게 자주 그려진 줄은 남는다.
///
/// **② 흘러간 줄 수만큼 다시 그려졌나** — 화면이 한 줄 흐를 때마다 다시 그려지는 행은
/// 내용이 아니라 장식이다. 내용은 한 번 찍히고 흘러간다.
///
/// 자가 하나였을 때 새는 자리가 있었다. 아래 두 줄만 덮어 그리는 프로그램은 **덮어 그리는
/// 행이 전부 장식**이라 중앙값 자체가 높아지고, 그러면 아무것도 걸리지 않는다. 표본이
/// 적으면 판단을 접는 가드(`< 4`)도 같은 방향으로 틀렸다 — 적게 그리는 프로그램일수록
/// 그 몇 줄이 장식일 가능성이 크다.
///
/// ## 윈도우에서는 이 셈이 반쪽이다 (실측, 2026-08-30)
///
/// 표식을 박은 기록으로 재 봤다. 프로그램은 `ESC[30;3H` 와 `ESC[31;3H` 를 **240번** 보냈는데
/// 우리 기록에 남은 커서 이동은 **20번**뿐이었고, 그마저 다른 행(24)이었다.
///
/// 까닭은 ConPTY 다. **윈도우에서 우리가 받는 것은 프로그램이 보낸 escape 가 아니라
/// ConPTY 가 자기 화면 버퍼를 보고 다시 그린 것이다.** 그래서 "프로그램이 어느 행을 몇 번
/// 그렸나"를 escape 로 세면 실제보다 훨씬 적게 나온다.
///
/// 그 기록에서는 내용 121줄을 전부 살렸지만 장식 19줄이 함께 남았다. 자를 하나 더 대도
/// 그대로였다 — 셀 신호 자체가 기록에 없기 때문이다. 진짜 클로드 코드 기록(12MB)으로는
/// 두 자 사이의 차이가 0.6% 뿐이었으니(26,641 → 26,484줄) 더해서 손해는 없다.
///
/// 제대로 고치려면 escape 를 세는 대신 **펴 내면서 행마다 몇 번 덮였는지 세야** 한다.
/// 그것은 이 함수를 두 번 도는 구조를 바꾸는 일이라 별도 배치로 미룬다.
fn chrome_rows(s: &str) -> std::collections::HashSet<usize> {
    let mut count: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\u{1b}' || it.next() != Some('[') {
            continue;
        }
        let (p, last) = read_csi(&mut it);
        if last == 'H' || last == 'f' {
            *count.entry(p.first().copied().unwrap_or(0)).or_default() += 1;
        }
    }
    if count.is_empty() {
        return Default::default(); // 커서를 옮겨 그린 적이 없다 — 판단할 것이 없다.
    }
    let mut nums: Vec<usize> = count.values().copied().collect();
    nums.sort_unstable();
    let median = nums[nums.len() / 2];
    let by_median = (median * 8).max(100);
    // 화면이 몇 줄 흘렀는가 — 그만큼 다시 그려진 행은 내용이 아니다.
    // 절반으로 잡는다: 매 줄마다는 아니어도 꾸준히 다시 그려지면 장식이다.
    let scrolled = s.matches('\n').count();
    let by_scroll = (scrolled / 2).max(10);
    count
        .into_iter()
        .filter(|(_, n)| *n > by_median || *n >= by_scroll)
        .map(|(r, _)| r.saturating_sub(1))
        .collect()
}

/// 기록 파일을 읽는다 — **쓰이는 중에도 읽을 수 있어야 한다.**
///
/// `read_to_string` 을 쓰면 안 된다. 기록은 지금도 덧붙여지고 있어서 파일 끝에 **여러
/// 바이트짜리 글자의 앞부분만** 남아 있는 순간이 흔하다. 그러면 UTF-8 로 못 읽는다며
/// 통째로 실패한다 — 사용자에게는 "전체 기록 열기가 안 된다"로 보인다.
/// 실제로 그렇게 됐다(2026-08-30, 12MB 기록에서 재현).
///
/// 잘린 끝은 버리면 된다. 형식이 한 줄에 하나씩인 JSON 이라 마지막 줄이 깨져도
/// 그 줄만 빠진다(`sessioncastread::parse_cast` 가 이미 그렇게 동작한다).
pub(crate) fn read_log(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// 기록 파일 전체를 읽을 글로 바꾼다.
///
/// 덧붙여 쓰는 동안에도 부를 수 있다. 마지막 줄이 잘려 있으면 그 줄만 빠진다.
pub(crate) fn cast_to_plain(text: &str) -> String {
    let joined: String =
        crate::sessioncastread::parse_cast(text).into_iter().map(|(_, s)| s).collect();
    flatten(&joined)
}

#[cfg(test)]
#[path = "castplain_tests.rs"]
mod tests;
