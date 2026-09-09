//! 직렬 포트 **설정**과 포트 목록 — 순수한 부분만.
//!
//! 여는 일은 [`crate::open`] 이 하고, 여기서는 "무엇으로 열 것인가"만 다룬다. 갈라 두면
//! 포트가 없는 기계에서도 규칙을 시험할 수 있다.

/// 흔히 쓰는 속도. 목록 순서가 곧 화면 순서다.
///
/// 장비 콘솔은 9600 이 압도적이고(시스코·주니퍼 기본), 그다음이 115200 이다.
/// 두 값을 앞에 두면 대부분은 고를 일이 없다.
pub const BAUDS: &[u32] = &[9600, 115200, 19200, 38400, 57600, 4800, 2400, 230400, 460800, 921600];

/// 기본 속도 — 장비 콘솔 케이블의 사실상 표준.
pub const DEFAULT_BAUD: u32 = 9600;

/// 한 포트를 여는 설정.
///
/// 기본은 **9600 8N1** 이다. 이 조합이 아니면 못 붙는 장비가 대부분이고, 다르게 쓰는
/// 장비는 설명서에 그 값을 적어 둔다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerialCfg {
    pub baud: u32,
    pub data_bits: u8,
    /// `N`(없음)·`E`(짝수)·`O`(홀수).
    pub parity: char,
    /// 1 또는 2.
    pub stop_bits: u8,
}

impl Default for SerialCfg {
    fn default() -> Self {
        Self { baud: DEFAULT_BAUD, data_bits: 8, parity: 'N', stop_bits: 1 }
    }
}

impl SerialCfg {
    /// `8N1` 처럼 사람이 읽고 쓰는 꼴.
    ///
    /// 장비 설명서가 늘 이 표기를 쓴다 — 우리가 다른 말로 옮기면 대조하기 어려워진다.
    pub fn frame(&self) -> String {
        format!("{}{}{}", self.data_bits, self.parity, self.stop_bits)
    }

    /// `8N1` 을 읽는다. 모양이 아니면 None(기본값으로 조용히 넘어가지 않는다).
    pub fn parse_frame(s: &str) -> Option<(u8, char, u8)> {
        let c: Vec<char> = s.trim().to_ascii_uppercase().chars().collect();
        if c.len() != 3 {
            return None;
        }
        let data = c[0].to_digit(10)? as u8;
        let stop = c[2].to_digit(10)? as u8;
        if !(5..=8).contains(&data) || !(1..=2).contains(&stop) || !"NEO".contains(c[1]) {
            return None;
        }
        Some((data, c[1], stop))
    }
}

/// 화면에 보일 포트 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortInfo {
    /// `COM3` 같은 이름.
    pub name: String,
    /// 무엇이 꽂혀 있는지(USB 어댑터 이름 등). 모르면 빈 글.
    pub detail: String,
}

/// 포트 이름을 **사람이 세는 차례**로 정렬한다.
///
/// 글자 그대로 정렬하면 `COM10` 이 `COM2` 앞에 온다. 포트가 열 개를 넘는 기계
/// (USB 어댑터를 여러 개 꽂은 자리)에서는 목록이 뒤죽박죽으로 보인다.
pub fn sort_ports(ports: &mut [PortInfo]) {
    ports.sort_by_key(|p| natural_key(&p.name));
}

/// `COM10` → `("COM", 10)`. 숫자가 없으면 0 으로 본다.
fn natural_key(name: &str) -> (String, u64) {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (head, tail) = name.split_at(name.len() - digits);
    (head.to_ascii_uppercase(), tail.parse().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(n: &str) -> PortInfo {
        PortInfo { name: n.into(), detail: String::new() }
    }

    /// 사람이 세는 차례여야 한다 — 글자 정렬이면 COM10 이 COM2 앞에 온다.
    #[test]
    fn 포트는_숫자_차례로_선다() {
        let mut v = vec![p("COM10"), p("COM2"), p("COM1"), p("COM21")];
        sort_ports(&mut v);
        let names: Vec<&str> = v.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["COM1", "COM2", "COM10", "COM21"]);
    }

    /// 숫자가 없는 이름(리눅스 `/dev/ttyUSB` 같은 것)도 터지지 않아야 한다.
    #[test]
    fn 숫자가_없어도_괜찮다() {
        let mut v = vec![p("ttyS"), p("COM3")];
        sort_ports(&mut v);
        assert_eq!(v.len(), 2);
    }

    /// `8N1` 은 장비 설명서의 표기다 — 오갔다 와도 같아야 한다.
    #[test]
    fn 프레임_표기가_오간다() {
        let c = SerialCfg::default();
        assert_eq!(c.frame(), "8N1");
        assert_eq!(SerialCfg::parse_frame("8N1"), Some((8, 'N', 1)));
        assert_eq!(SerialCfg::parse_frame("7e2"), Some((7, 'E', 2)));
    }

    /// 모양이 아니면 **거절한다.** 조용히 기본값으로 넘어가면 사용자가 적은 것과
    /// 다른 설정으로 붙어서, 글자가 깨져 나오는데 이유를 알 수 없다.
    #[test]
    fn 이상한_표기는_거절한다() {
        for bad in ["", "8N", "8N11", "9N1", "8X1", "8N3", "4N1", "abc"] {
            assert_eq!(SerialCfg::parse_frame(bad), None, "{bad}");
        }
    }

    /// 기본은 장비 콘솔의 사실상 표준이어야 한다.
    #[test]
    fn 기본은_구천육백_팔엔일() {
        let c = SerialCfg::default();
        assert_eq!((c.baud, c.frame().as_str()), (9600, "8N1"));
        assert_eq!(BAUDS[0], DEFAULT_BAUD, "목록 맨 앞이 기본값이어야 고를 일이 없다");
    }
}
