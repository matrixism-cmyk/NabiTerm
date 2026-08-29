//! 웹 탭을 **밖에서 읽고 조종하는** 길 — `nabi cli web-list` / `web-eval`.
//!
//! ## 왜 필요한가
//!
//! 지금까지 웹 탭에 할 수 있는 일은 여는 것뿐이었다. pane 안의 AI 는 무엇이 떠 있는지도,
//! 그 쪽에 무엇이 적혔는지도 알 수 없었다 — AI 에게는 없는 기능이나 마찬가지였다.
//!
//! WebView2 는 쪽 안에서 자바스크립트를 실행하고 그 값을 돌려주는 길을 갖고 있다. 그것을
//! 제어 평면에 내놓는다. 그러면 AI 가 `document.body.innerText` 로 읽고,
//! `document.querySelector(...).click()` 로 누를 수 있다.
//!
//! ## 왜 답을 기다리지 않고 나중에 보내는가
//!
//! 스크립트 결과는 화면이 만들어 준다. 그 화면은 UI 실에서 돈다. 여기서 붙잡고 기다리면
//! UI 실이 멈추고, 멈춘 실은 답을 만들 수 없다 — 서로 붙잡는다. 그래서 답이 오는 때에
//! `Event::WebResult` 로 보낸다. 부른 쪽은 그 사건을 기다린다.
//!
//! ## 어느 탭인가
//!
//! 번호를 주지 않으면, 열려 있는 웹 탭이 **하나일 때만** 그것을 쓴다. 여럿일 때 아무거나
//! 고르면 엉뚱한 쪽에 코드를 넣게 된다 — 그때는 번호를 대라고 한다.

use nabi_proto::Event;
use nabi_types::PaneId;

impl crate::app::NabiApp {
    /// 열려 있는 웹 탭 목록을 JSON 으로 돌려준다.
    pub(crate) fn control_web_list(&mut self, seq: u64) {
        let items: Vec<String> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(_, p)| self.web_tabs.get(p).map(|w| (p, w)))
            .map(|(p, w)| {
                format!(
                    r#"{{"pane":{},"url":{},"title":{}}}"#,
                    p.get(),
                    json_str(&w.url),
                    json_str(&w.title)
                )
            })
            .collect();
        self.reply_web(seq, true, format!("[{}]", items.join(",")));
    }

    /// 웹 탭 안에서 자바스크립트를 실행하고, 답이 오면 보낸다.
    pub(crate) fn control_web_eval(&mut self, seq: u64, pane: Option<u64>, js: String) {
        let target = match self.pick_web_tab(pane) {
            Ok(p) => p,
            Err(msg) => return self.reply_web(seq, false, msg),
        };
        let Some(tab) = self.web_tabs.get(&target) else {
            return self.reply_web(seq, false, format!("웹 탭 {} 이 없다", target.get()));
        };
        let Some(view) = &tab.view else {
            // 아직 한 번도 안 그려졌으면 화면 자체가 없다 — 탭을 눌러 보이게 해야 만들어진다.
            return self.reply_web(seq, false, "그 웹 탭은 아직 화면이 없다(탭을 한 번 보여 줄 것)".into());
        };
        let hub = self.control_events.clone();
        view.eval(&js, move |r| {
            let (ok, data) = match r {
                Ok(json) => (true, json),
                Err(e) => (false, e),
            };
            hub.publish(&Event::WebResult { seq, ok, data });
        });
    }

    /// 어느 웹 탭에 물을 것인가.
    pub(crate) fn pick_web_tab(&self, pane: Option<u64>) -> Result<PaneId, String> {
        if let Some(n) = pane {
            let p = PaneId::new(n);
            return match self.web_tabs.contains_key(&p) {
                true => Ok(p),
                false => Err(format!("웹 탭 {n} 이 없다 (web-list 로 번호를 볼 것)")),
            };
        }
        let mut it = self.web_tabs.keys();
        match (it.next(), it.next()) {
            (Some(p), None) => Ok(*p),
            (None, _) => Err("열려 있는 웹 탭이 없다 (nabi cli web --url … 로 먼저 열 것)".into()),
            // 여럿이면 고르지 않는다 — 엉뚱한 쪽에 코드를 넣는 것보다 묻는 편이 낫다.
            _ => Err("웹 탭이 여럿이다 — --pane <번호> 로 지목할 것 (web-list 참조)".into()),
        }
    }

    pub(crate) fn reply_web(&self, seq: u64, ok: bool, data: String) {
        self.control_events.publish(&Event::WebResult { seq, ok, data });
    }
}

/// 글 하나를 JSON 문자열로 감싼다.
///
/// 쪽 제목에는 따옴표도 역슬래시도 줄바꿈도 들어온다. 그대로 붙이면 받는 쪽에서 JSON 이
/// 깨진다 — 실제로 제목에 따옴표가 든 쪽은 흔하다.
pub(crate) fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::json_str;

    #[test]
    fn 따옴표와_역슬래시를_막는다() {
        assert_eq!(json_str(r#"a"b\c"#), r#""a\"b\\c""#);
        assert_eq!(json_str("줄\n바꿈"), r#""줄\n바꿈""#);
        // 제어 문자도 새어 나가면 안 된다.
        assert_eq!(json_str("\u{1}"), r#""\u0001""#);
    }

    #[test]
    fn 보통_글은_그대로다() {
        assert_eq!(json_str("GitHub \u{2014} nabiTerm"), "\"GitHub \u{2014} nabiTerm\"");
    }
}
