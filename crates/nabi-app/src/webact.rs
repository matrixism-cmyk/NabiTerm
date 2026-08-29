//! 웹 탭을 **밖에서 조종하는** 아홉 가지 — `nabi cli web-back` 부터 `web-pdf` 까지.
//!
//! ## 왜 `web-eval` 로는 부족한가
//!
//! 자바스크립트를 넣는 길은 이미 있다. 하지만 쪽 안에서 할 수 없는 일이 있다.
//!
//! * 자기 로딩을 멈추는 것.
//! * 뒤로/앞으로 가는 것 — `history.back()` 은 되지만 **갈 곳이 있는지** 물을 수 없다.
//! * 확대 배율 — 브라우저가 갖는 값이지 쪽이 갖는 값이 아니다.
//! * 자기 모습을 그림이나 PDF 로 남기는 것.
//!
//! ## `web-text` 는 왜 있는가
//!
//! `web-eval --js "document.body.innerText"` 로도 된다. 그런데 AI 가 웹 탭에 하는 일 중
//! 가장 잦은 것이 "이 쪽에 뭐라고 적혀 있냐"다. 자주 쓰는 것에 짧은 이름을 주면 그만큼
//! 덜 틀린다 — 따옴표를 겹겹이 감싸다 깨지는 일이 없다.
//!
//! ## 답은 언제 오는가
//!
//! 전부 `Event::WebResult` 로 돌아간다. 그림과 PDF 는 엣지가 다 쓴 뒤에 알려 주므로
//! 그때 보낸다 — 파일이 다 만들어지기 전에 "됐다"고 하면 부른 쪽이 빈 파일을 연다.

use nabi_proto::Event;

impl crate::app::NabiApp {
    /// 웹 조종 하나를 수행한다.
    pub(crate) fn control_web_act(
        &mut self,
        seq: u64,
        pane: Option<u64>,
        act: String,
        arg: String,
    ) {
        let target = match self.pick_web_tab(pane) {
            Ok(p) => p,
            Err(msg) => return self.reply_web(seq, false, msg),
        };
        let Some(view) = self.web_tabs.get(&target).and_then(|t| t.view.as_ref()) else {
            // 아직 한 번도 안 그려졌으면 화면 자체가 없다 — 탭을 눌러 보이게 해야 만들어진다.
            return self
                .reply_web(seq, false, "그 웹 탭은 아직 화면이 없다(탭을 한 번 보여 줄 것)".into());
        };
        let hub = self.control_events.clone();
        match act.as_str() {
            // 되돌아갈 곳이 없는데 시키면 조용히 아무 일도 안 일어난다 — 그 편이 헷갈린다.
            "back" | "forward" => {
                let (can, go): (bool, fn(&nabi_web::embed::Embedded)) = match act.as_str() {
                    "back" => (view.can_back(), |v| v.back()),
                    _ => (view.can_forward(), |v| v.forward()),
                };
                match can {
                    true => {
                        go(view);
                        self.reply_web(seq, true, format!("\"{act}\""));
                    }
                    false => self.reply_web(seq, false, format!("{act} 로 갈 곳이 없다")),
                }
            }
            "reload" => {
                view.reload();
                self.reply_web(seq, true, "\"reload\"".into());
            }
            "stop" => {
                view.stop();
                self.reply_web(seq, true, "\"stop\"".into());
            }
            "goto" => {
                view.go(&arg);
                self.reply_web(seq, true, crate::webctl::json_str(&arg));
            }
            "zoom" => match arg.trim().parse::<f64>() {
                Ok(z) => {
                    view.set_zoom(z);
                    self.reply_web(seq, true, format!("{:.3}", view.zoom()));
                }
                Err(_) => self.reply_web(seq, false, format!("배율이 숫자가 아니다: {arg}")),
            },
            // 쪽의 글만 — AI 가 가장 자주 묻는 것이라 짧은 이름을 줬다.
            "text" => view.eval("document.body.innerText", move |r| {
                let (ok, data) = match r {
                    Ok(json) => (true, json),
                    Err(e) => (false, e),
                };
                hub.publish(&Event::WebResult { seq, ok, data });
            }),
            "shot" => {
                let out = self.web_out_path(&arg, "png");
                let note = out.clone();
                view.capture_png(&out, move |r| {
                    let (ok, data) = match r {
                        Ok(()) => (true, crate::webctl::json_str(&note)),
                        Err(e) => (false, e),
                    };
                    hub.publish(&Event::WebResult { seq, ok, data });
                });
            }
            "pdf" => {
                let out = self.web_out_path(&arg, "pdf");
                let note = out.clone();
                view.print_pdf(&out, move |r| {
                    let (ok, data) = match r {
                        Ok(()) => (true, crate::webctl::json_str(&note)),
                        Err(e) => (false, e),
                    };
                    hub.publish(&Event::WebResult { seq, ok, data });
                });
            }
            other => self.reply_web(seq, false, format!("모르는 동작: {other}")),
        }
    }

    /// 저장할 자리. 안 주면 임시 폴더에 짓는다 — 어디에 뒀는지는 답으로 알려 준다.
    ///
    /// 빈손으로 실패시키지 않는 까닭은, 부르는 쪽이 대개 "일단 찍어 보고 싶다"이기
    /// 때문이다. 자리를 꼭 정하게 하면 그때마다 임시 경로를 스스로 지어야 한다.
    fn web_out_path(&self, arg: &str, ext: &str) -> String {
        if !arg.trim().is_empty() {
            return arg.trim().to_string();
        }
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("nabi-web-{ms}.{ext}")).display().to_string()
    }
}
