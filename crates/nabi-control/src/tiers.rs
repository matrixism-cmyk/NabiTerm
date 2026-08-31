//! **낱말 하나가 어느 승인 등급인가** — 설명서가 자기를 검사할 때 물어보는 단 하나의 자리.
//!
//! ## 왜 이것이 필요한가
//!
//! 도움말 ▸ AI 제어에는 낱말이 등급별로 나뉘어 적혀 있다. 그 표를 손으로 관리했더니
//! 코드와 어긋났다 — `web-eval`·`quit`·`update` 가 "늘 허용" 칸에 앉아 있었고
//! `web-list` 는 두 칸에 겹쳐 있었다. **AI 는 그 표를 읽고 계획을 세운다.** 승인이 필요한
//! 낱말을 필요 없다고 읽으면, 세워 둔 계획이 중간에 "approval pending" 으로 멈춘다.
//!
//! 그래서 표를 검사하는 시험이 여기에 물어본다. 등급을 정하는 코드가 바뀌면 표가 자동으로
//! 틀리게 되고, 시험이 그 자리에서 잡는다.
//!
//! ## 네 등급
//!
//! - `read` — 보기만 한다. 어떤 모드에서도 승인 없이 된다.
//! - `act` — 무언가를 바꾼다. `ask` 모드에서 한 번 승인이 필요하다.
//! - `inject` — 남의 pane 에 글자를 밀어 넣거나 되돌릴 수 없는 일을 한다. 따로 승인한다.
//! - `local` — 서버에 가지 않고 이 프로세스에서 끝난다(설명서 출력 같은 것).

use crate::protocol::ControlRequest;

/// `nabi cli` 뒤에 오는 인자들을 주면 그 낱말의 등급을 답한다. 모르는 낱말이면 `None`.
fn tier_of(args: &[String]) -> Option<&'static str> {
    let (first, second) = (args.first()?.as_str(), args.get(1).map(String::as_str));
    // 서버에 가지 않는 것들 — 정책 게이트를 지나지 않으므로 등급이랄 게 없다.
    match (first, second) {
        ("layout", Some("apply")) | ("security", Some("audit")) | ("api", Some("schema")) => {
            return Some("local")
        }
        ("integration", _) => return Some("local"),
        // 글자를 밀어 넣고 기다린다 — 요청 두 개로 나뉘지만 무게는 주입이다.
        ("agent", Some("prompt")) => return Some("inject"),
        _ => {}
    }
    let req = crate::clientverbs::parse_verb(args).ok()?;
    Some(tier_of_request(&req))
}

/// 요청 하나의 등급. 읽기 전용은 정책 게이트에 닿기 전에 갈라지므로 여기서 먼저 걸러 낸다.
fn tier_of_request(req: &ControlRequest) -> &'static str {
    // 이 목록은 `dispatch::dispatch` 의 앞쪽 갈래 + `server::handle_conn` 이 가로채는 셋과
    // 짝이다. 한쪽만 늘리면 등급이 어긋나므로 함께 고칠 것.
    let read = matches!(
        req,
        ControlRequest::Hello { .. }
            | ControlRequest::ListPanes
            | ControlRequest::PaneModes { .. }
            | ControlRequest::Capture { .. }
            | ControlRequest::AgentExplain { .. }
            | ControlRequest::Wait { .. }
            | ControlRequest::Tail { .. }
            | ControlRequest::Subscribe { .. }
    );
    if read {
        return "read";
    }
    match crate::dispatch::group_of(req) {
        crate::policy::Group::Inject => "inject",
        _ => "act",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Vec<String> {
        s.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn the_four_tiers_come_out_where_they_should() {
        assert_eq!(tier_of(&v("list")), Some("read"));
        assert_eq!(tier_of(&v("capture --pane 1")), Some("read"));
        assert_eq!(tier_of(&v("focus --pane 1")), Some("act"));
        assert_eq!(tier_of(&v("send --pane 1 --data hi")), Some("inject"));
        assert_eq!(tier_of(&v("api schema")), Some("local"));
    }

    /// 예전에 설명서가 틀리게 적어 두었던 넷. 여기서 못을 박아 둔다.
    #[test]
    fn the_ones_the_guide_used_to_get_wrong() {
        assert_eq!(tier_of(&v("web-eval --js 1")), Some("inject"), "코드를 넣는 일이다");
        assert_eq!(tier_of(&v("quit")), Some("inject"), "되돌릴 수 없다");
        assert_eq!(tier_of(&v("update")), Some("inject"), "프로그램을 바꿔 친다");
        assert_eq!(tier_of(&v("update --check")), Some("act"), "확인만 하면 바꾸는 게 없다");
        assert_eq!(tier_of(&v("restart")), Some("inject"), "끄고 다시 켠다 — quit 보다 더 한다");
    }

    #[test]
    fn a_word_we_do_not_have_gets_no_answer() {
        assert_eq!(tier_of(&v("bogus-verb")), None);
        assert_eq!(tier_of(&[]), None);
    }

    /// 설명서가 적어 둔 문서용 인자를 **실제로 파싱되는 인자**로 바꾼다.
    ///
    /// `capture --pane <id> [--lines <n>]` 를 그대로 넘기면 `<id>` 가 숫자가 아니라 파싱이
    /// 실패한다. 그래서 `<…>` 는 값 하나로 바꾸고, `[…]` 로 감싼 선택 항목은 통째로 뺀다.
    fn sample_args(spec: &str) -> Vec<String> {
        let head = spec.split('`').next().unwrap_or_default();
        let (mut out, mut brackets, mut in_angle) = (String::new(), 0i32, false);
        for c in head.chars() {
            match c {
                '[' => brackets += 1,
                ']' => brackets = (brackets - 1).max(0),
                _ if brackets > 0 => {}
                '<' => {
                    in_angle = true;
                    out.push('1');
                }
                '>' => in_angle = false,
                _ if in_angle => {}
                '"' => {} // 따옴표는 셸이 벗겨 주므로 인자에는 남지 않는다.
                _ => out.push(c),
            }
        }
        out.split_whitespace().map(str::to_string).collect()
    }

    /// **AI 에게 주는 설명서의 등급이 우리가 실제로 강제하는 등급과 같은가.**
    ///
    /// 설명서는 `nabi-app` 에 있지만 시험은 여기 있다 — 등급을 정하는 코드가 여기이기 때문이다.
    /// 반대로 두면 `tier_of` 를 크레이트 밖으로 열어야 하는데, 그러면 시험 하나 때문에 공개된
    /// 함수가 생긴다.
    ///
    /// 예전에는 설명서가 등급을 **제목으로** 주장했다("Inspect (always allowed)"). 그 아래
    /// `web-eval`·`quit`·`update` 가 앉아 있었는데 셋 다 별도 승인이 필요하다. AI 는 이 표를
    /// 읽고 계획을 세우므로, 틀린 표는 계획을 중간에 세운다.
    #[test]
    fn the_guide_we_hand_to_agents_states_the_tiers_we_enforce() {
        let guide = include_str!("../../nabi-app/src/agentguide.rs");
        let mut wrong = Vec::new();
        let mut seen = 0;
        for line in guide.lines() {
            let Some(rest) = line.trim_start().strip_prefix("- (") else { continue };
            let Some((tier, spec)) = rest.split_once(") `nabi cli ") else { continue };
            seen += 1;
            let args = sample_args(spec);
            match tier_of(&args) {
                Some(real) if real == tier => {}
                Some(real) => wrong.push(format!("{}: 적힌 등급 {tier}, 실제 {real}", args.join(" "))),
                None => wrong.push(format!("{}: 파싱 불가 — sample_args 를 고칠 것", args.join(" "))),
            }
        }
        assert!(seen > 40, "설명서에서 낱말을 {seen}개밖에 못 읽었다 — 모양이 바뀌었다");
        assert!(wrong.is_empty(), "설명서의 등급이 코드와 다르다:\n  {}", wrong.join("\n  "));
    }
}
