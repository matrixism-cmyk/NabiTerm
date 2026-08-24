//! AI CLI 설치·제거 명령을 **출력을 읽어 가며** 실행한다 — 진행 막대가 멈춰 보이지 않게.
//!
//! 예전에는 `Command::output()`으로 명령이 끝날 때까지 통째로 기다렸다. npm 전역 설치는
//! 몇 분이 걸리는데 그동안 막대는 65%에 붙박이였다. 사용자가 창을 닫아 버릴 만하다
//! (사용자 지적 2026-08-25 — "한세월 기다리다가는 다들 창을 닫아버릴껄").
//!
//! ## 진행률을 정직하게 만드는 법
//!
//! npm은 "지금 몇 %"를 알려 주지 않는다. 없는 숫자를 지어내는 대신 세 가지를 겹친다:
//!
//! 1. **단계 구간** — 각 단계에 `[lo, hi)` 구간을 배정한다(Node 설치 · CLI 설치 …).
//! 2. **출력이 곧 진척** — 한 줄(또는 `\r` 한 번) 나올 때마다 남은 구간의 일부만큼 다가간다.
//!    점근이라 `hi`에 닿지는 않는다 — 실제로 끝나야 다음 단계로 넘어간다.
//! 3. **지금 무슨 일이 일어나는지** — 마지막 출력 줄과 경과 시간을 그대로 보여 준다.
//!    막대만 보면 못 믿어도, 흐르는 글자와 초는 살아 있다는 증거가 된다.
//!
//! `\r`도 줄 끝으로 본다. npm·curl은 진행 표시를 같은 줄에 덮어쓰므로, `\n`만 기다리면
//! 몇 분 동안 아무 것도 못 읽는다.

use crate::aicli::{set_progress, ActionJob};
use std::io::Read;
use std::process::{Output, Stdio};
use std::time::Instant;

/// 한 번 읽을 때마다 남은 구간의 이만큼을 좁힌다. 작을수록 천천히, 오래 걸려도 안 넘친다.
const STEP: f32 = 0.04;

/// 출력을 흘려 읽으며 진행률을 갱신하는 PowerShell 실행.
///
/// `lo`~`hi`는 이 명령이 차지할 진행 막대 구간이다. `label`은 진행 중 앞에 붙는 말.
pub(crate) fn run_ps(job: &ActionJob, script: &str, lo: f32, hi: f32, label: &str) -> std::io::Result<Output> {
    let mut child = crate::aicli::hidden("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // stderr도 같이 읽어야 한다 — npm은 진행 표시와 경고를 전부 stderr로 보낸다.
    let err = child.stderr.take().map(reader_thread);
    let mut frac = lo;
    let started = Instant::now();
    if let Some(out) = child.stdout.take() {
        let mut out = out;
        let (mut buf, mut line) = ([0u8; 4096], String::new());
        loop {
            let n = match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            for &b in &buf[..n] {
                if b == b'\n' || b == b'\r' {
                    note(job, &mut frac, hi, label, &line, started);
                    line.clear();
                } else if line.len() < 400 {
                    line.push(b as char);
                }
            }
        }
        note(job, &mut frac, hi, label, &line, started);
    }
    let status = child.wait()?;
    let stderr = err.and_then(|h| h.join().ok()).unwrap_or_default();
    Ok(Output { status, stdout: Vec::new(), stderr })
}

/// stderr를 통째로 모아 두는 곁 스레드 — 실패했을 때 이유를 보여 주려면 필요하다.
fn reader_thread(mut r: std::process::ChildStderr) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = r.read_to_end(&mut v);
        v
    })
}

/// 출력 한 조각을 받아 진행률을 한 걸음 옮기고, 지금 무슨 일인지 적는다.
fn note(job: &ActionJob, frac: &mut f32, hi: f32, label: &str, line: &str, started: Instant) {
    let t = line.trim();
    *frac += (hi - *frac) * STEP;
    let secs = started.elapsed().as_secs();
    let tail = if t.is_empty() { String::new() } else { format!(" \u{2014} {}", clip(t, 60)) };
    set_progress(job, *frac, format!("{label} ({secs}s){tail}"));
}

/// 긴 줄은 잘라 준다 — 막대 안 글씨가 창을 밀어내지 않게.
fn clip(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}\u{2026}", &s[..i]),
        None => s.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> ActionJob {
        std::sync::Arc::new(std::sync::Mutex::new(Default::default()))
    }

    /// 출력이 아무리 많아도 배정된 구간을 넘지 않아야 한다 — 넘으면 100%인데 안 끝난 꼴이 된다.
    #[test]
    fn progress_creeps_toward_the_ceiling_but_never_past_it() {
        let (j, mut f) = (job(), 0.2f32);
        for i in 0..500 {
            note(&j, &mut f, 0.9, "설치 중", &format!("line {i}"), Instant::now());
        }
        assert!(f < 0.9, "구간 상한을 넘었다: {f}");
        assert!(f > 0.85, "500줄이나 나왔는데 거의 안 움직였다: {f}");
    }

    /// 진행률은 뒤로 가지 않는다.
    #[test]
    fn progress_never_goes_backwards() {
        let (j, mut f) = (job(), 0.0f32);
        let mut prev = f;
        for _ in 0..50 {
            note(&j, &mut f, 1.0, "x", "y", Instant::now());
            assert!(f >= prev);
            prev = f;
        }
    }

    /// 막대 글씨에는 경과 시간이 들어가야 한다 — 숫자가 멈춰도 초는 흐른다.
    #[test]
    fn the_message_shows_what_is_happening_and_for_how_long() {
        let (j, mut f) = (job(), 0.0f32);
        note(&j, &mut f, 1.0, "Node.js 설치 중", "added 120 packages", Instant::now());
        let m = j.lock().unwrap().message.clone();
        assert!(m.contains("Node.js 설치 중"), "{m}");
        assert!(m.contains("(0s)"), "{m}");
        assert!(m.contains("added 120 packages"), "{m}");
    }

    #[test]
    fn a_very_long_output_line_is_clipped() {
        let long = "x".repeat(300);
        let (j, mut f) = (job(), 0.0f32);
        note(&j, &mut f, 1.0, "L", &long, Instant::now());
        assert!(j.lock().unwrap().message.chars().count() < 90);
    }
}
