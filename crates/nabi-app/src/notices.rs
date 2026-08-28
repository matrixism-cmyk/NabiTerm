//! **한 번만 알리는 것들**(배치 AH) — 조용히 안 되고 있는 기능을 사용자에게 한 번 말한다.
//!
//! 오늘 배치 AB·AF 를 지나며 같은 모양이 셋이 됐다. 셋 다 `trailui`(에이전트 기록 창)에
//! 얹혀 있었는데, 거기는 창을 그리는 파일이지 이런 판단이 살 자리가 아니다.
//!
//! ## 왜 "한 번만"인가
//!
//! 셋 다 **반복되는 상황**이다. 에이전트는 막혀도 계속 시도하고, 같은 서버로 500개를 보내면
//! 검증 실패도 500번이며, 규칙 파일은 프로그램이 사는 동안 그대로다. 매번 알리면 곧 읽지
//! 않게 되고, **읽지 않는 알림은 없느니만 못하다.**
//!
//! 한 번이면 "아, 그게 안 되고 있구나"를 떠올리기에 충분하다. 자세한 것은 각자의 창에서 본다.
//!
//! ## 왜 여기 모았는가
//!
//! 같은 판단(반복되는 조용한 실패를 한 번만 알린다)이 세 곳에 흩어져 있으면, 네 번째를 더할
//! 사람은 그 규칙이 있다는 것을 모른다. 한곳에 모아 두면 **다음 사람이 규칙을 먼저 본다.**

use crate::app::NabiApp;
use nabi_i18n::tr;
use std::time::Instant;

impl NabiApp {
    /// 매 프레임 한 번 — 셋을 차례로 묻는다.
    ///
    /// 부르는 쪽(`update`)이 목록을 들고 있으면, 네 번째를 더할 때 **거기도 고쳐야 한다는
    /// 것을 잊는다.** 목록은 규칙과 같은 자리에 둔다.
    ///
    /// 셋 다 이미 알렸으면 불 값 세 번 읽기로 끝난다 — 매 프레임 불러도 싸다.
    pub(crate) fn tick_notices(&mut self) {
        self.notice_first_denial();
        self.notice_verify_skipped();
        self.notice_dropped_rules();
    }

    /// 에이전트 요청이 **처음 막혔을 때 한 번만** 알린다(배치 AB T1).
    ///
    /// "ask" 모드에는 승인 대화상자가 있어 사용자가 안다. 문제는 **"off"** 다 — 그때는
    /// 대화상자도 없이 조용히 막히고, 사용자는 에이전트가 왜 아무것도 못 하는지 모른다.
    ///
    /// 매번 알리지 않는 이유: 자율 에이전트는 막혀도 계속 시도한다. 매번 띄우면 곧
    /// 읽지 않게 되고, 그러면 없느니만 못하다. **처음 한 번**이면 "아, 꺼 뒀지"를 떠올리기에
    /// 충분하다. 자세한 것은 행동 기록 창에서 본다.
    pub(crate) fn notice_first_denial(&mut self) {
        if self.denial_noticed || nabi_control::trail::denied_total() == 0 {
            return;
        }
        self.denial_noticed = true;
        self.notify = Some((tr(self.lang, "trail.denied.first").to_string(), Instant::now()));
    }

    /// 해시 검증을 **못 하고 넘어간** 전송이 처음 생겼을 때 한 번만 알린다(배치 AF).
    ///
    /// 검증을 켜 둔 사용자는 전송이 검증됐다고 믿는다. 그런데 서버에 해시 명령이 없으면
    /// (윈도우 OpenSSH 등) 우리는 조용히 건너뛰었다 — 화면은 검증된 전송과 똑같아 보였다.
    /// **신뢰가 전부인 기능에서 그것이 가장 나쁜 실패다.**
    ///
    /// 매번 알리지 않는 이유는 거부 알림과 같다: 같은 서버로 500개를 보내면 500번 뜬다.
    /// 한 번이면 "이 서버는 검증이 안 되는구나"를 알기에 충분하다.
    pub(crate) fn notice_verify_skipped(&mut self) {
        if self.verify_skip_noticed || nabi_sftp::hashcheck::tally().1 == 0 {
            return;
        }
        self.verify_skip_noticed = true;
        self.notify = Some((tr(self.lang, "sftp.verify.skipped").to_string(), Instant::now()));
    }

    /// 사용자가 쓴 감지 규칙이 깨져 버려졌으면 한 번만 알린다(배치 AF).
    ///
    /// 규칙 하나가 깨졌다고 나머지를 못 쓰게 하지는 않는다 — 그건 손해다. 하지만 **몇 개가
    /// 사라졌는지 말하지 않으면** 그 사람은 자기 규칙이 왜 안 걸리는지 알 방법이 없다.
    /// 내장 규칙은 시험이 지키므로 여기 세어지는 것은 사실상 사용자가 쓴 것뿐이다.
    pub(crate) fn notice_dropped_rules(&mut self) {
        if self.rules_drop_noticed || self.agent_watch.dropped == 0 {
            return;
        }
        self.rules_drop_noticed = true;
        let msg = format!("{} {}", tr(self.lang, "rules.dropped"), self.agent_watch.dropped);
        self.notify = Some((msg, Instant::now()));
    }
}
