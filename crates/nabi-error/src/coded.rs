//! **말이 아니라 코드로 나르는 오류.**
//!
//! ## 왜 필요한가
//!
//! 지금까지 아래쪽 크레이트들은 오류를 한국어 문장으로 만들어 올려 보냈다.
//!
//! ```text
//! format!("셸 실행파일을 찾을 수 없습니다: {} (설치되어 있지 않음)", name)
//! ```
//!
//! 그 문장은 **거기서 이미 한국어로 굳는다.** 영어나 일본어로 쓰는 사람은 프로그램 전체가
//! 자기 말인데 오류만 한국어로 뜬다. 실측으로 그런 자리가 170곳이었다(2026-09-05).
//!
//! 게다가 같은 오류를 **두 곳에서 다르게 보여 줘야** 하는 경우가 있다. `nabi cli` 의
//! 오류는 AI 에이전트가 읽으므로 화면 언어와 무관하게 영어여야 하고, 화면의 알림은
//! 사용자 언어여야 한다. 문장으로 굳혀 버리면 그 둘을 나눌 수 없다.
//!
//! ## 그래서 코드로 나른다
//!
//! 오류가 생긴 자리는 **무슨 일인지**만 적는다(`shell.notfound` 와 그 인자). 어떤 말로
//! 보여 줄지는 **보여 주는 자리**가 정한다. 낮은 크레이트는 화면 언어를 몰라도 된다 —
//! 알 이유도 없다.
//!
//! ## 번역이 없어도 읽을 수는 있어야 한다
//!
//! `Display` 는 `shell.notfound: powershell.exe` 처럼 코드와 인자를 그대로 적는다.
//! 예쁘지는 않지만 **무슨 일인지는 알 수 있고**, 로그에 남았을 때 검색도 된다.
//! 번역을 빠뜨렸다고 오류가 통째로 사라지는 일은 없다.

use std::fmt;

/// 코드로 나르는 오류 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coded {
    /// 무슨 일인가 — i18n 목록의 `err.<code>` 를 가리킨다. 점으로 나눈 소문자.
    pub code: &'static str,
    /// 문구에 끼워 넣을 값들. 번역문의 `{0}` `{1}` 자리에 순서대로 들어간다.
    pub args: Vec<String>,
}

impl Coded {
    /// 인자 없는 오류.
    pub fn new(code: &'static str) -> Self {
        Self { code, args: Vec::new() }
    }

    /// 인자를 붙인 오류.
    pub fn with(code: &'static str, args: impl IntoIterator<Item = String>) -> Self {
        Self { code, args: args.into_iter().collect() }
    }

    /// 한 개짜리 인자 — 가장 흔한 모양이라 짧게 쓸 수 있게 둔다.
    pub fn one(code: &'static str, arg: impl fmt::Display) -> Self {
        Self { code, args: vec![arg.to_string()] }
    }

    /// 번역문에 인자를 끼워 넣는다.
    ///
    /// `{0}` `{1}` 처럼 **번호로** 적는다. 이름표(`{name}`)를 쓰지 않는 까닭은, 번역하는
    /// 사람이 이름을 옮겨 적다 틀리면 그 자리가 조용히 빈 채로 나가기 때문이다. 번호는
    /// 옮겨 적을 것이 없다.
    ///
    /// 자리표시자가 모자라면 남는 인자는 뒤에 괄호로 붙인다 — 값을 잃는 것보다 낫다.
    pub fn fill(&self, template: &str) -> String {
        let mut out = template.to_string();
        let mut used = 0usize;
        for (i, a) in self.args.iter().enumerate() {
            let ph = format!("{{{i}}}");
            if out.contains(&ph) {
                out = out.replace(&ph, a);
                used += 1;
            }
        }
        if used < self.args.len() {
            let rest: Vec<&str> = self.args[used..].iter().map(String::as_str).collect();
            out.push_str(&format!(" ({})", rest.join(", ")));
        }
        out
    }
}

/// 번역이 없을 때의 모습 — 코드와 인자를 그대로.
impl fmt::Display for Coded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.args.is_empty() {
            true => write!(f, "{}", self.code),
            false => write!(f, "{}: {}", self.code, self.args.join(", ")),
        }
    }
}

impl std::error::Error for Coded {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 번역이_없으면_코드를_그대로_보여_준다() {
        assert_eq!(Coded::new("shell.notfound").to_string(), "shell.notfound");
        assert_eq!(
            Coded::one("shell.notfound", "pwsh.exe").to_string(),
            "shell.notfound: pwsh.exe"
        );
    }

    #[test]
    fn 번호_자리에_인자를_끼운다() {
        let e = Coded::with("x", ["가".into(), "나".into()]);
        assert_eq!(e.fill("{0} 다음에 {1}"), "가 다음에 나");
        // 순서를 바꿔도 된다 — 말마다 어순이 다르다.
        assert_eq!(e.fill("{1} 앞에 {0}"), "나 앞에 가");
    }

    /// 번역문이 자리표시자를 빠뜨렸다고 **값을 잃으면 안 된다.**
    ///
    /// 어느 말의 번역 하나가 `{0}` 을 빠뜨렸을 때, 그 말을 쓰는 사람만 "무엇이" 없는지
    /// 모르게 된다. 그런 어긋남은 조용해서 오래 간다.
    #[test]
    fn 자리표시자가_모자라면_뒤에_붙인다() {
        let e = Coded::one("x", "powershell.exe");
        assert_eq!(e.fill("셸을 찾지 못했습니다"), "셸을 찾지 못했습니다 (powershell.exe)");
    }

    #[test]
    fn 인자가_없으면_그대로다() {
        assert_eq!(Coded::new("x").fill("그냥 문장"), "그냥 문장");
    }

    /// 같은 자리표시자가 여러 번 나오면 모두 채운다.
    #[test]
    fn 같은_자리가_여러_번이면_전부_채운다() {
        let e = Coded::one("x", "A");
        assert_eq!(e.fill("{0} 와 {0}"), "A 와 A");
    }
}
