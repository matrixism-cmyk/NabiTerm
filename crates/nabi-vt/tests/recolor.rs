//! **글자를 칠 때마다 줄을 다시 칠하는 셸**을 우리가 제대로 그리는가.
//!
//! ## 왜 이 시험이 있나
//!
//! "파워셸에서 `|` 나 `@` 를 치면 프롬프트가 일부 깨져 보인다"는 보고를 받았다
//! (2026-09-05). 세션 기록에서 실제 바이트를 잡아 보니, 그 글자들은 보통 글자와 하는
//! 일이 달랐다.
//!
//! ```text
//! 보통 글자:  ESC[93m Get-Proc                       ← 뒤에 덧붙이기만
//! | 를 칠 때: ESC[93m ESC[24;7H Get-Process ESC[m | ESC[2C
//!                     ^^^^^^^^^                ^^^^
//!                     되돌아가서            색을 바꿔 다시 칠한다
//! ```
//!
//! `|` 와 `@` 는 앞 낱말을 **명령 자리**로 만든다. PSReadLine 이 그 낱말을 다시 칠하려고
//! 커서를 되돌려 줄을 통째로 다시 그린다. 그 다시 그리기를 우리가 어떻게 그리는지를
//! 여기서 못 박는다 — 화면으로는 순간이라 잡을 수 없다.

use nabi_types::GridSize;
use nabi_vt::TermModel;

/// 그 줄이 화면에서 어떻게 보이는가(뒤 공백은 뗀다).
fn line(m: &TermModel, row: usize) -> String {
    let all = m.dump_text(200);
    all.lines().nth(row).unwrap_or("").trim_end().to_string()
}

/// 되돌아가 다시 칠해도 **글자는 그대로** 남아야 한다.
#[test]
fn 되돌아가_다시_칠해도_글자는_그대로다() {
    let mut m = TermModel::new(GridSize::new(80, 24), 100);
    // 프롬프트 + 낱말을 찍는다.
    m.process(b"PS C:\\> Get-Process");
    assert_eq!(line(&m, 0), "PS C:\\> Get-Process");
    // 셸이 되돌아가 같은 자리에 색만 바꿔 다시 칠하고 `|` 를 덧붙인다.
    // 9열 = 프롬프트 "PS C:\\> " 여덟 글자 다음.
    m.process(b"\x1b[93m\x1b[1;9HGet-Process\x1b[m|\x1b[2C");
    assert_eq!(line(&m, 0), "PS C:\\> Get-Process|", "다시 칠한 뒤 줄이 달라졌다");
}

/// `@` 도 같은 길을 탄다 — 스플랫 자리라 앞 낱말을 다시 칠한다.
#[test]
fn 골뱅이도_같다() {
    let mut m = TermModel::new(GridSize::new(80, 24), 100);
    m.process(b"PS C:\\> Write-Host ");
    m.process(b"\x1b[93m\x1b[1;9HWrite-Host \x1b[m@\x1b[2C");
    assert_eq!(line(&m, 0), "PS C:\\> Write-Host @");
}

/// 줄 끝까지 공백으로 밀어 지우는 다시 그리기 — ConPTY 가 실제로 이렇게 보낸다.
///
/// 폭을 꽉 채워 보내므로 **마지막 칸을 넘어가는 순간**이 있다. 넘어가며 다음 줄로
/// 넘어가 버리면 화면이 한 줄씩 밀린다.
#[test]
fn 폭을_꽉_채워_다시_그려도_줄이_안_밀린다() {
    let mut m = TermModel::new(GridSize::new(20, 5), 100);
    m.process(b"\x1b[1;1Habc");
    // 20칸을 꽉 채운다(17칸 공백 + 3글자 = 20).
    m.process(b"\x1b[1;1Hxyz                 ");
    assert_eq!(line(&m, 0), "xyz", "꽉 채운 다시 그리기가 줄을 밀었다");
    // 둘째 줄은 건드리지 않았으니 비어 있어야 한다.
    assert_eq!(line(&m, 1), "");
}

/// 색 신호가 길어도(참색 앞뒤 배경까지) 글자는 그대로여야 한다.
///
/// PSReadLine 은 `ESC[0;38;2;R;G;B;48;2;R;G;Bm` 처럼 열한 개짜리를 보낸다.
#[test]
fn 긴_색_신호가_글자를_먹지_않는다() {
    let mut m = TermModel::new(GridSize::new(40, 5), 100);
    m.process(b"\x1b[0;38;2;221;221;221;48;2;30;30;30mPS C:\\> \x1b[0;38;2;245;245;67;48;2;30;30;30mGet-Proc\x1b[0m");
    assert_eq!(line(&m, 0), "PS C:\\> Get-Proc");
}
