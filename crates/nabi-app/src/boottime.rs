//! 시작 시간 **계측** — 느려졌을 때 언제부터인지 알 수 있게.
//!
//! 감사(2026-08-25) 결과 시작 경로에 계측이 **하나도 없었다.** 상용 제품은 자기 성능을
//! 안다. 계측이 없으면 "요즘 좀 느린 것 같다"는 보고에 대고 추측만 하게 된다.
//!
//! 재는 것은 세 구간이다 — 프로세스가 뜬 뒤 우리 `main`까지(런타임·로더), 설정을 읽고
//! 창을 만들기까지(우리 준비), 그리고 **첫 프레임이 실제로 그려질 때까지**(사용자가 창을
//! 본 순간). 마지막 것이 사용자가 체감하는 시간이다.
//!
//! 결과는 로그로만 남긴다 — 화면에 띄우면 그 자체가 방해다. 필요하면 도움말▸진단 로그에서
//! 본다(`logview`).

use std::time::Instant;

/// 시작 구간을 재는 시계. `main` 맨 앞에서 만든다.
pub(crate) struct Boot {
    started: Instant,
    /// 창 옵션까지 준비된 시각.
    ready: Option<Instant>,
}

impl Boot {
    pub(crate) fn start() -> Self {
        Self { started: Instant::now(), ready: None }
    }

    /// 창을 띄우기 직전(설정 읽기·로그 초기화·그래픽 선택이 끝난 시점).
    pub(crate) fn window_ready(&mut self) {
        self.ready = Some(Instant::now());
        tracing::info!(target: "boot", ms = self.started.elapsed().as_millis(), "준비 완료(창 생성 직전)");
    }

    /// 첫 프레임이 그려진 시점 — **사용자가 창을 본 순간**.
    pub(crate) fn first_frame(&self) {
        let total = self.started.elapsed().as_millis();
        let gfx = self.ready.map(|r| r.elapsed().as_millis());
        match gfx {
            Some(g) => tracing::info!(target: "boot", total_ms = total, graphics_ms = g, "첫 프레임"),
            None => tracing::info!(target: "boot", total_ms = total, "첫 프레임"),
        }
        // 눈에 띄게 느리면 경고로 남긴다 — 로그를 훑을 때 바로 보이게.
        if is_slow(total) {
            tracing::warn!(target: "boot", total_ms = total, "시작이 느립니다");
        }
    }
}

/// 이 정도를 넘으면 "느리다"고 본다(ms).
///
/// 터미널은 눌러서 바로 떠야 하는 프로그램이다. 2초는 사용자가 뭔가 잘못됐다고 느끼기
/// 시작하는 선이고, 그 아래는 굳이 시끄럽게 하지 않는다.
const SLOW_MS: u128 = 2000;

fn is_slow(total_ms: u128) -> bool {
    total_ms > SLOW_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_genuinely_slow_start_is_flagged() {
        assert!(!is_slow(0));
        assert!(!is_slow(SLOW_MS));
        assert!(is_slow(SLOW_MS + 1));
    }

    /// 창 준비를 알리지 않아도 첫 프레임 기록이 터지지 않아야 한다(경로가 갈릴 수 있다).
    #[test]
    fn recording_the_first_frame_without_a_ready_mark_is_fine() {
        Boot::start().first_frame();
    }

    #[test]
    fn the_ready_mark_is_never_before_the_start() {
        let mut b = Boot::start();
        b.window_ready();
        let ready = b.ready.expect("창 준비 시각이 찍혀야 한다");
        // "0보다 크다"고 물으면 안 된다. 두 줄 사이에 시계 눈금이 한 번도 안 넘어가면
        // 0이 나와서 **아무 잘못이 없는데도** 실패한다(2026-08-29에 실제로 그랬다).
        //
        // 우리가 정말 알고 싶은 것은 시계가 뒤로 가지 않는가다. 그것만 묻는다.
        assert!(ready >= b.started);
    }
}
