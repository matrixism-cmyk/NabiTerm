//! 전송 큐 UI — 항목별 일시정지·순서변경·제거·재시도(WinSCP 큐 모델).
//!
//! 큐가 길어져도 파일 목록을 밀어내지 않도록 높이를 제한한 스크롤 영역에 그린다.

use crate::sftpxfer::{eta_secs, human_secs, xfer_totals, Transfer, XferState};
use nabi_i18n::{tr, Lang};

/// 큐 목록이 차지할 수 있는 최대 높이(px). 넘으면 그 안에서 스크롤한다.
const QUEUE_MAX_H: f32 = 150.0;

/// 큐에서 사용자가 요청한 동작(처리는 sftpact).
#[derive(Default)]
pub(crate) struct QueueAct {
    /// 끝난 항목 비우기.
    pub clear: bool,
    /// 진행 중인 전송 전부 중단.
    pub cancel_all: bool,
    /// 항목 제거(대기·정지·완료) 또는 중단(진행 중).
    pub remove: Option<u64>,
    /// 일시정지 ↔ 재개 토글.
    pub toggle_pause: Option<u64>,
    /// 대기 항목 순서 이동(-1=위, +1=아래).
    pub move_by: Option<(u64, i32)>,
    /// 실패 항목 재시도.
    pub retry: Option<u64>,
}

/// 진행 중 항목들의 (진행률%, 합산 속도, 남은 시간) 요약 문구.
fn summary(transfers: &[Transfer]) -> String {
    let items: Vec<(u64, u64, u64)> = transfers
        .iter()
        .filter(|t| t.running())
        .map(|t| {
            let s = t.started.elapsed().as_secs_f64();
            let sp = if s > 0.3 && t.bytes > 0 { (t.bytes as f64 / s) as u64 } else { 0 };
            (t.bytes, t.size, sp)
        })
        .collect();
    if items.is_empty() {
        return String::new();
    }
    let (b, sz, sp) = xfer_totals(&items);
    let pct = (b * 100).checked_div(sz).unwrap_or(0);
    let eta = if sp > 0 && sz > b { format!(" \u{00b7} ~{}", human_secs((sz - b) / sp)) } else { String::new() };
    format!("  \u{2211}{pct}% \u{00b7} {}/s{eta}", crate::browserfs::human(sp))
}

/// 전체 크기를 모르는 전송(폴더)의 진행 표시 — 보낸 양 + 속도.
fn running_amount(t: &Transfer) -> String {
    let sent = crate::browserfs::human(t.bytes);
    let secs = t.started.elapsed().as_secs_f64();
    if secs <= 0.3 || t.bytes == 0 {
        return sent;
    }
    format!("{sent} ({}/s)", crate::browserfs::human((t.bytes as f64 / secs) as u64))
}

/// 진행 중 항목의 라벨(이름 + 속도 + 남은 시간).
fn running_label(t: &Transfer, dir: &str) -> String {
    let secs = t.started.elapsed().as_secs_f64();
    if secs <= 0.3 || t.bytes == 0 {
        return format!("{dir} {}", t.name);
    }
    let sp = (t.bytes as f64 / secs) as u64;
    let speed = crate::browserfs::human(sp);
    match eta_secs(t.bytes, t.size, secs) {
        Some(e) => format!("{dir} {} ({speed}/s \u{00b7} ~{})", t.name, human_secs(e)),
        None => format!("{dir} {} ({speed}/s)", t.name),
    }
}

/// 전송 큐를 그리고 사용자 동작을 모은다.
pub(crate) fn show_queue(ui: &mut egui::Ui, transfers: &[Transfer], lang: Lang) -> QueueAct {
    let mut act = QueueAct::default();
    if transfers.is_empty() {
        return act;
    }
    let done = transfers.iter().filter(|t| t.state.finished()).count();
    let waiting = transfers.iter().filter(|t| !t.state.finished() && !t.running()).count();
    ui.separator();
    ui.horizontal(|ui| {
        let mut head = format!("{} {done}/{}", tr(lang, "sftp.transfers"), transfers.len());
        if waiting > 0 {
            head.push_str(&format!(" \u{00b7} {} {waiting}", tr(lang, "sftp.q.waiting")));
        }
        ui.label(head + &summary(transfers));
        if done > 0 && ui.small_button("\u{1f5d1}").on_hover_text(tr(lang, "sftp.q.clear")).clicked() {
            act.clear = true;
        }
        if transfers.iter().any(|t| t.running())
            && ui.small_button("\u{23f9}").on_hover_text(tr(lang, "sftp.q.cancelall")).clicked()
        {
            act.cancel_all = true;
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("sftp_queue")
        .max_height(QUEUE_MAX_H)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            // 성공한 항목은 줄로 남기지 않는다 — 쌓이면서 정작 보고 싶은 진행 중 항목을
            // 아래로 밀어낸다. 개수는 위 머리글에 이미 있고, 실패만 손댈 일이 남는다.
            for t in transfers.iter().filter(|t| t.state != XferState::Done) {
                row(ui, t, lang, &mut act);
            }
        });
    act
}

/// 큐 한 줄 — 상태에 맞는 표시 + 그 상태에서 의미 있는 버튼만.
fn row(ui: &mut egui::Ui, t: &Transfer, lang: Lang, act: &mut QueueAct) {
    let dir = if t.up { "\u{2b06}" } else { "\u{2b07}" };
    ui.horizontal(|ui| {
        match t.state {
            XferState::Running if t.size > 0 => {
                let frac = (t.bytes as f32 / t.size as f32).clamp(0.0, 1.0);
                ui.add(egui::ProgressBar::new(frac).desired_width(180.0).text(running_label(t, dir)));
            }
            // 폴더 전송은 전체 크기를 미리 모른다(재귀로 훑어야 안다). 막대 대신 지금까지
            // 보낸 양과 속도를 보여 준다 — 예전엔 모래시계만 있어서 멈춘 것과 구별이 안 됐다.
            XferState::Running => {
                ui.label(format!("{dir} {} \u{23f3} {}", t.name, running_amount(t)));
            }
            XferState::Waiting => {
                ui.weak(format!("{dir} {} \u{00b7} {}", t.name, crate::browserfs::human(t.size)));
            }
            XferState::Paused => {
                ui.weak(format!("\u{23f8} {} \u{00b7} {}", t.name, crate::browserfs::human(t.size)));
            }
            XferState::Done => {
                ui.label(format!("{dir} {} \u{2713}  {}", t.name, crate::browserfs::human(t.size)));
            }
            XferState::Failed => {
                let l = ui.colored_label(crate::theme_ui::ERR, format!("{dir} {} \u{2717}", t.name));
                if !t.err.is_empty() {
                    l.on_hover_text(&t.err);
                }
            }
        }
        buttons(ui, t, lang, act);
    });
}

/// 상태별 조작 버튼(그 상태에서 할 수 있는 것만 보여준다).
fn buttons(ui: &mut egui::Ui, t: &Transfer, lang: Lang, act: &mut QueueAct) {
    match t.state {
        XferState::Waiting | XferState::Paused => {
            let (icon, hint) = match t.state {
                XferState::Paused => ("\u{25b6}", "sftp.q.resume"),
                _ => ("\u{23f8}", "sftp.q.pause"),
            };
            if ui.small_button(icon).on_hover_text(tr(lang, hint)).clicked() {
                act.toggle_pause = Some(t.xfer);
            }
            if ui.small_button("\u{2191}").on_hover_text(tr(lang, "sftp.q.up")).clicked() {
                act.move_by = Some((t.xfer, -1));
            }
            if ui.small_button("\u{2193}").on_hover_text(tr(lang, "sftp.q.down")).clicked() {
                act.move_by = Some((t.xfer, 1));
            }
        }
        XferState::Failed if ui.small_button("\u{21bb}").on_hover_text(tr(lang, "sftp.retry")).clicked() => {
            act.retry = Some(t.xfer);
        }
        _ => {}
    }
    // 제거는 어느 상태에서나 가능하다(진행 중이면 중단 후 제거).
    if ui.small_button("\u{2715}").on_hover_text(tr(lang, "sftp.q.remove")).clicked() {
        act.remove = Some(t.xfer);
    }
}

#[cfg(test)]
mod tests {
    use crate::sftpxfer::{Transfer, XferState};

    fn q() -> Vec<Transfer> {
        let mut v = vec![
            Transfer::new(1, "a".into(), false, 100),
            Transfer::new(2, "b".into(), false, 100),
            Transfer::new(3, "c".into(), false, 100),
        ];
        v[0].state = XferState::Running;
        v
    }

    /// 대기 항목만 순서를 바꾼다 — 진행 중인 항목을 밀어내면 이미 보낸 명령과 큐가 어긋난다.
    #[test]
    fn reorder_moves_within_waiting() {
        let mut v = q();
        crate::sftpqact::move_waiting(&mut v, 3, -1);
        assert_eq!(v.iter().map(|t| t.xfer).collect::<Vec<_>>(), vec![1, 3, 2]);
        // 진행 중 항목 위로는 올라가지 못한다.
        crate::sftpqact::move_waiting(&mut v, 3, -1);
        assert_eq!(v.iter().map(|t| t.xfer).collect::<Vec<_>>(), vec![1, 3, 2]);
    }

    #[test]
    fn reorder_down_stops_at_end() {
        let mut v = q();
        crate::sftpqact::move_waiting(&mut v, 3, 1);
        assert_eq!(v.iter().map(|t| t.xfer).collect::<Vec<_>>(), vec![1, 2, 3], "맨 끝이면 그대로");
        crate::sftpqact::move_waiting(&mut v, 2, 1);
        assert_eq!(v.iter().map(|t| t.xfer).collect::<Vec<_>>(), vec![1, 3, 2]);
    }

    #[test]
    fn finished_items_do_not_block() {
        let mut v = q();
        v[0].state = XferState::Failed;
        assert!(v[0].state.finished(), "실패도 끝난 것 — 큐를 막지 않는다");
        assert!(!v[1].state.finished());
    }
}
