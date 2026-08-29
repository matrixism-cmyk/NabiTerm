//! 에이전트가 무엇을 했는지 남긴다(배치 AB) — **내용은 남기지 않는다.**
//!
//! 2026년 에이전트 제어 평면의 표준 다섯 가지 중 넷은 이미 있다 — 정책(off/ask/on),
//! 그룹별 승인, 사람 승인, 정책 우회 금지(MCP도 같은 동사 경로를 탄다). 빠진 하나가 이것이다.
//!
//! 지금까지는 `tracing::info!(target: "control", …)` 뿐이었고, 그것은 `NABI_LOG` 를 켜야
//! 남는다. **기본값에서는 아무 흔적도 없다.** 여러 시간 자율로 도는 에이전트를 붙여 놓고
//! 나중에 "그때 뭘 했지?"에 답할 수 없으면 아무도 맡기지 않는다.
//!
//! ## 무엇을 남기고 무엇을 안 남기는가
//!
//! **동사·대상·결과만 남긴다. 내용은 담지 않는다.**
//!
//! `SendInput` 의 본문에는 비밀번호가 지나가고 `SpawnTerminal` 의 인자에도 토큰이 섞인다.
//! 이 저장소에는 `redact` 가 있지만, **가리는 것보다 아예 담지 않는 편이 낫다** — 가리기는
//! 규칙이 못 잡는 꼴이 늘 생기고, 감사 기록이 새 유출 경로가 되면 고치는 것보다 큰 일이 된다.
//!
//! 길이는 남긴다. "무언가를 보냈다"와 "얼마나 보냈다"는 사고를 되짚을 때 다르게 쓰인다.
//!
//! ## 왜 디스크에 안 쓰는가
//!
//! 파일로 두면 그 파일이 또 하나의 지워야 할 것이 된다. 메모리 고리에만 두고, 사용자가
//! 내보내기를 누를 때만 밖으로 나간다.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

/// 고리에 담는 최대 건수. 넘치면 오래된 것부터 밀려난다.
///
/// 자율 에이전트는 분당 수십 건을 낼 수 있다. 너무 작으면 사고 시점이 이미 밀려나 있고,
/// 너무 크면 메모리를 오래 붙든다. 몇 시간치를 되짚기에 충분한 선으로 잡았다.
const CAP: usize = 2000;

/// 요청 하나의 자취.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// 프로그램이 켜진 뒤 흐른 초 — 벽시계가 아니라 상대 시각이다.
    ///
    /// 벽시계를 쓰면 기록 자체가 "언제 무엇을 했는지"를 시간대까지 담게 되는데, 그것은
    /// 내보내기로 남에게 건넬 때 필요 이상을 알려 준다. 순서와 간격이면 되짚기에 충분하다.
    pub at_secs: u64,
    /// 요청한 곳(제어 평면이 아는 이름 — pane 번호나 "mcp" 등).
    pub from: String,
    /// 동사(`send-input`·`spawn`·`sftp-put`…).
    pub verb: &'static str,
    /// 대상 — pane 번호나 경로처럼 **이름**만. 내용은 넣지 않는다.
    pub target: String,
    /// 결과.
    pub outcome: Outcome,
    /// 보낸/받은 바이트 수(해당 없으면 0). 내용은 담지 않고 크기만 남긴다.
    pub bytes: usize,
}

/// 요청이 어떻게 끝났는가.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// 정책이 허용해 실행됐다.
    Allowed,
    /// 정책이 막았다. **거부도 반드시 남는다** — 무엇을 시도했는지가 감사의 절반이다.
    Denied,
    /// 사람에게 물어봤고 사람이 허락했다.
    Approved,
    /// 실행하다 실패했다(정책과 무관).
    Failed,
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Allowed => "allowed",
            Outcome::Denied => "denied",
            Outcome::Approved => "approved",
            Outcome::Failed => "failed",
        }
    }
}

/// 자취를 담는 고리. **시험이 자기 것을 만들 수 있게** 구조로 둔다.
///
/// 전역 하나만 두면 시험들이 같은 고리를 공유해 서로 간섭한다(넘침 시험이 순서 시험의
/// 자취를 밀어낸다). 그런 시험은 혼자 돌리면 통과하고 함께 돌리면 깨져서, 진짜 결함처럼
/// 보이는 데 시간을 쓰게 된다 — 오늘 이미 한 번 겪었다.
pub struct Trail {
    ring: VecDeque<Entry>,
    cap: usize,
}

impl Trail {
    pub fn new(cap: usize) -> Self {
        Self { ring: VecDeque::new(), cap: cap.max(1) }
    }

    /// 자취를 넣는다. 넘치면 **오래된 것부터** 밀려난다 — 최신이 사라지면 사고 직후를 못 본다.
    pub fn push(&mut self, e: Entry) {
        if self.ring.len() >= self.cap {
            self.ring.pop_front();
        }
        self.ring.push_back(e);
    }

    /// 최신이 뒤에 오는 순서.
    pub fn entries(&self) -> Vec<Entry> {
        self.ring.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.ring.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }
}

fn global() -> &'static Mutex<Trail> {
    static R: OnceLock<Mutex<Trail>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Trail::new(CAP)))
}

/// 지금까지 **거부된 건수**(줄지 않는다). 화면이 매 프레임 물어도 싸도록 세어만 둔다.
///
/// 고리 전체를 훑어 세면 프레임마다 수천 건을 돌게 된다. 기록을 보려고 만든 것이
/// 프로그램을 느리게 만들면 안 된다.
static DENIED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// 거부 누적 건수.
pub fn denied_total() -> usize {
    DENIED.load(std::sync::atomic::Ordering::Relaxed)
}


/// 자취를 남긴다. 잠금이 오염돼도 계속 쓴다 — 기록 때문에 제어가 멈추면 안 된다.
pub fn record(e: Entry) {
    if e.outcome == Outcome::Denied {
        DENIED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    global().lock().unwrap_or_else(|p| p.into_inner()).push(e);
}

/// 최신이 뒤에 오는 순서로 전부 돌려준다.
pub fn entries() -> Vec<Entry> {
    global().lock().unwrap_or_else(|p| p.into_inner()).entries()
}

/// 담긴 건수.
pub fn len() -> usize {
    global().lock().map(|r| r.len()).unwrap_or(0)
}

/// 사람이 읽고 붙여넣을 수 있는 한 덩어리로 만든다.
///
/// 물어보는 사람에게 보여 줄 수 있어야 기록이 쓸모가 있다. 탭으로 나눠 표 모양이라
/// 그대로 붙여넣어도 읽히고, 표 계산기에도 들어간다.
pub fn export(list: &[Entry]) -> String {
    let mut out = String::from("time(s)	from	verb	target	outcome	bytes
");
    for e in list {
        out.push_str(&format!(
            "{}	{}	{}	{}	{}	{}
",
            e.at_secs, e.from, e.verb, e.target, e.outcome.label(), e.bytes
        ));
    }
    out
}


/// 요청에서 **남길 것만** 뽑는다 — `(동사, 대상, 바이트)`.
///
/// 여기가 이 모듈의 심장이다. `SendInput` 의 본문이나 `SpawnTerminal` 의 명령줄에는
/// 비밀번호·토큰이 지나간다. 그래서 **내용은 한 글자도 담지 않는다** — 무엇을 향해
/// 무엇을 했는지와 크기만 남긴다.
///
/// 경로는 남긴다. 어떤 파일을 가져갔는지는 감사의 핵심이고, 경로 자체가 비밀인 경우는
/// 내용이 비밀인 경우보다 훨씬 드물다.
pub fn describe(req: &crate::protocol::ControlRequest) -> (&'static str, String, usize) {
    use crate::protocol::ControlRequest as R;
    // 포괄 갈래(`_ =>`)를 두지 않는다. 새 동사를 더하면 **컴파일러가 여기를 짚어** 준다 —
    // 실제로 이 표를 처음 적을 때 dispatch.rs 만 훑어 세 동사를 빠뜨렸고, 컴파일러가 잡았다.
    // 포괄 갈래가 있었으면 그 셋은 이름 없이 기록됐을 것이다.
    match req {
        // 본문은 **길이만**. 여기에 비밀번호가 지나간다.
        R::SendInput { pane, data, .. } => ("send-input", format!("pane {pane}"), data.len()),
        R::ClosePane { pane } => ("close-pane", format!("pane {pane}"), 0),
        R::Resize { pane, cols, rows } => ("resize", format!("pane {pane} {cols}x{rows}"), 0),
        R::Focus { pane } => ("focus", format!("pane {pane}"), 0),
        R::SetTitle { pane, .. } => ("set-title", format!("pane {pane}"), 0),
        // 상태 값은 담지 않는다 — 에이전트가 아무 글이나 넣을 수 있는 자리다. 열쇠만 남긴다.
        R::PaneStatusSet { key, .. } => ("pane-status", key.clone(), 0),
        // 셸 이름만 — 명령줄은 담지 않는다.
        R::SpawnTerminal { ssh: Some(s), .. } => ("spawn-ssh", s.clone(), 0),
        R::SpawnTerminal { shell, .. } => ("spawn", format!("{shell:?}"), 0),
        R::OpenBrowser { path } => ("open-browser", path.clone().unwrap_or_default(), 0),
        R::OpenHere { path } => ("open-here", path.clone(), 0),
        // 주소는 남긴다 — 에이전트가 어디를 열었는지가 이 기록의 요점이다.
        R::OpenWeb { url } => ("web", url.clone().unwrap_or_default(), 0),
        // 찍은 자리만 남기고 파일 경로는 남기지 않는다 — 그림 자체가 남의 데이터일 수 있다.
        R::Screenshot { pane, .. } => (
            "screenshot",
            pane.map(|p| format!("pane {p}")).unwrap_or_else(|| "창 전체".into()),
            0,
        ),
        R::Progress { pane, percent } => (
            "progress",
            match percent {
                Some(p) => format!("pane {pane} {p}%"),
                None => format!("pane {pane} 지움"),
            },
            0,
        ),
        R::OpenEditor { path } => ("open-editor", path.clone(), 0),
        R::OpenSftp { session } => ("open-sftp", session.clone(), 0),
        R::SftpList { path } => ("sftp-list", path.clone(), 0),
        R::SftpGet { remote, .. } => ("sftp-get", remote.clone(), 0),
        R::SftpPut { remote, .. } => ("sftp-put", remote.clone(), 0),
        // 알림 제목·본문은 사용자 화면에 뜨는 글이라 담지 않는다(남의 데이터일 수 있다).
        R::Notify { .. } => ("notify", String::new(), 0),
        R::LayoutExport => ("layout-export", String::new(), 0),
        R::ScheduleCreate { .. } => ("schedule-create", String::new(), 0),
        // 읽기 동사는 이 갈래로 오지 않는다. 와도 이름은 남긴다.
        R::Hello { .. } => ("hello", String::new(), 0),
        R::ListPanes => ("list", String::new(), 0),
        R::PaneModes { pane } => ("pane-modes", format!("pane {pane}"), 0),
        R::Capture { pane, .. } => ("capture", format!("pane {pane}"), 0),
        R::AgentExplain { pane } => ("explain", format!("pane {pane}"), 0),
        R::Wait { pane, .. } => ("wait", format!("pane {pane}"), 0),
        R::Tail { pane, .. } => ("tail", format!("pane {pane}"), 0),
        R::Subscribe { .. } => ("subscribe", String::new(), 0),
    }
}

/// 프로그램이 켜진 뒤 흐른 초.
fn uptime_secs() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs()
}

/// 자취 한 줄을 남긴다(전역 고리에).
pub fn note(from: Option<u64>, verb: &'static str, target: String, bytes: usize, outcome: Outcome) {
    record(Entry {
        at_secs: uptime_secs(),
        from: from.map(|p| format!("pane {p}")).unwrap_or_else(|| "mcp".into()),
        verb,
        target,
        outcome,
        bytes,
    });
}
