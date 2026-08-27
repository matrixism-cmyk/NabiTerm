//! SFTP 격자 보기(목록·아이콘·타일) — **보이는 줄만 그린다**(배치 Z F1·F2).
//!
//! `sftpview.rs`에서 떼어냈다. 가상화가 들어오면서 화면 계산·셀 그리기·줄 수 시험이 함께
//! 늘었고, 한 파일에 두면 다음에 고칠 사람이 어느 쪽을 건드리는지 헷갈린다.
//!
//! ## 왜 자동 줄바꿈을 쓰지 않는가
//!
//! 예전에는 `horizontal_wrapped`에 맡기고 항목을 **전부** 그렸다. 자세히 보기는 이미
//! `body.rows`로 보이는 행만 그리고 있었으므로, 같은 폴더를 자세히 보기로 열면 빠르고
//! 목록 보기로 열면 느렸다 — 사용자는 보기 모드를 바꿨을 뿐인데 프로그램이 달라졌다.
//!
//! 이제 한 줄에 몇 칸인지 **우리가 정한다**(`grid_shape`). egui의 자동 줄바꿈에 맡기면
//! 우리가 센 줄 수와 실제로 그려진 줄 수가 어긋날 수 있고, 그러면 스크롤 위치가 밀린다.
//! 이 어긋남은 작은 폴더에서는 티가 안 나고 큰 폴더에서만 드러나 찾기 어렵다.

use crate::sftpentries::EClick;
use crate::sftpview::{actions, icon, RemoteName};
use nabi_i18n::Lang;
use nabi_proto::SftpEntry;

/// 격자/타일/목록: 셀 크기·글자 크기로 모양을 조절(with_size면 크기도 표시).
#[allow(clippy::too_many_arguments)]
pub(crate) fn grid(
    ui: &mut egui::Ui,
    entries: &[&SftpEntry],
    cur: &str,
    lang: Lang,
    cw: f32,
    ch: f32,
    txt: f32,
    with_size: bool,
    // 이 항목이 선택되었는가(단일 선택 + 다중 선택 합치기).
    selected: &dyn Fn(&str) -> bool,
) -> Option<EClick> {
    let mut click = None;
    let big = ch > 40.0; // 아이콘 모드: 아이콘 위 / 이름 아래(2줄).
    // 한 줄에 몇 칸이 들어가는지 **우리가 정한다.** egui의 자동 줄바꿈에 맡기면 우리가 센
    // 줄 수와 실제로 그려진 줄 수가 어긋날 수 있고, 그러면 스크롤 위치가 밀린다.
    // 이 어긋남은 작은 폴더에서는 티가 안 나고 큰 폴더에서만 드러나 찾기 어렵다.
    let sp = ui.spacing().item_spacing;
    let bar = ui.spacing().scroll.bar_width + ui.spacing().scroll.bar_inner_margin;
    let has_up = cur != "/" && cur != ".";
    let cells = entries.len() + usize::from(has_up);
    let (per_row, rows) = grid_shape(ui.available_width() - bar, cw, sp.x, cells);
    // 셀 높이는 **인자로 받은 값**이라 재거나 짐작하지 않는다. 가상화가 조용히 깨지는
    // 가장 흔한 이유가 행 높이 추정인데, 여기서는 그 위험이 아예 없다.
    egui::ScrollArea::vertical()
        .id_salt("sftp_grid")
        .show_rows(ui, ch + sp.y, rows, |ui, range| {
            for r in range {
                ui.horizontal(|ui| {
                    let from = r * per_row;
                    for c in 0..per_row {
                        let idx = from + c;
                        if idx >= cells {
                            break;
                        }
                        // 맨 앞 ".." 셀: 더블클릭으로 상위 디렉터리 이동.
                        if has_up && idx == 0 {
                            let up = egui::RichText::new("\u{2b06} ..").size(txt);
                            if ui.add_sized([cw, ch], egui::Button::new(up)).double_clicked() {
                                click = Some(EClick::Nav(crate::sftppath::parent_dir(cur)));
                            }
                            continue;
                        }
                        let e = entries[idx - usize::from(has_up)];
                        cell(ui, e, cur, lang, cw, ch, txt, with_size, big, selected, &mut click);
                    }
                });
            }
        });
    click
}

/// 격자 셀 하나 — 아이콘·이름·크기, 선택 강조, 드래그 payload, 우클릭 동작.
///
/// 떼어낸 이유: 가상화가 들어오면서 그리는 자리가 두 겹 안으로 들어갔다. 그대로 두면
/// 한 함수가 화면 계산과 셀 그리기를 같이 하게 되고, 다음에 고칠 사람이 어느 쪽을
/// 건드리는지 헷갈린다.
#[allow(clippy::too_many_arguments)]
fn cell(
    ui: &mut egui::Ui,
    e: &SftpEntry,
    cur: &str,
    lang: Lang,
    cw: f32,
    ch: f32,
    txt: f32,
    with_size: bool,
    big: bool,
    selected: &dyn Fn(&str) -> bool,
    click: &mut Option<EClick>,
) {
    {
        {
            let sz = if with_size && !e.is_dir {
                format!("\n{}", crate::browserfs::human(e.size))
            } else {
                String::new()
            };
            let sep = if big { '\n' } else { ' ' };
            let color = if e.is_dir {
                crate::filetype::FOLDER_COLOR
            } else {
                crate::filetype::file_color(&e.name)
            };
            let label = egui::RichText::new(format!("{}{sep}{}{sz}", icon(e), e.name))
                .size(txt)
                .color(color);
            // click_and_drag로 직접 센스(드래그 커서로 클릭 막히는 문제 방지). 로컬 격자와 동일.
            let mut btn = egui::Button::new(label).sense(egui::Sense::click_and_drag());
            if selected(&e.name) {
                btn = btn.fill(ui.visuals().selection.bg_fill); // 선택 항목 강조.
            }
            let resp = ui.add_sized([cw, ch], btn);
            if resp.dragged() {
                resp.dnd_set_drag_payload(RemoteName {
                    name: e.name.clone(),
                    is_dir: e.is_dir,
                });
            }
            if let Some(a) = actions(&resp, e, cur, lang) {
                *click = Some(a);
            }
        }
    }
}

/// 격자의 모양 — (한 줄에 몇 칸, 모두 몇 줄).
///
/// 가상화가 조용히 깨지는 자리가 여기다. 우리가 센 줄 수와 실제로 그려진 줄 수가 어긋나면
/// 스크롤 위치가 밀리는데, 작은 폴더에서는 티가 안 나고 큰 폴더에서만 드러난다.
/// 그래서 화면 없이 확인할 수 있게 떼어 두고 시험을 붙였다.
pub(crate) fn grid_shape(avail_w: f32, cell_w: f32, gap: f32, cells: usize) -> (usize, usize) {
    // 칸이 n개면 사이 간격은 n-1개다. 그래서 (폭 + 간격) / (칸 + 간격).
    let per = (((avail_w + gap) / (cell_w + gap)).floor() as usize).max(1);
    (per, cells.div_ceil(per).max(1))
}

#[cfg(test)]
mod tests {
    use super::grid_shape;

    #[test]
    fn cells_fit_by_width_including_the_gaps() {
        // 폭 320, 칸 100, 간격 10 → 100+10+100+10+100 = 320. 딱 셋.
        assert_eq!(grid_shape(320.0, 100.0, 10.0, 9), (3, 3));
    }

    #[test]
    fn one_pixel_short_drops_a_column() {
        assert_eq!(grid_shape(319.0, 100.0, 10.0, 9).0, 2);
    }

    #[test]
    fn a_partial_last_row_still_counts() {
        // 일곱 개를 셋씩 → 세 줄(마지막 줄은 하나만).
        assert_eq!(grid_shape(320.0, 100.0, 10.0, 7), (3, 3));
    }

    #[test]
    fn never_zero_columns_even_in_a_sliver() {
        // 창을 아주 좁게 줄여도 0으로 나누지 않는다.
        assert_eq!(grid_shape(1.0, 100.0, 10.0, 5).0, 1);
        assert_eq!(grid_shape(0.0, 100.0, 10.0, 5).0, 1);
        assert_eq!(grid_shape(-50.0, 100.0, 10.0, 5).0, 1);
    }

    #[test]
    fn an_empty_folder_is_one_empty_row() {
        // 0줄이면 ScrollArea가 높이를 못 잡는다. 빈 폴더도 한 줄로 센다.
        assert_eq!(grid_shape(320.0, 100.0, 10.0, 0), (3, 1));
    }

    #[test]
    fn ten_thousand_files_do_not_overflow() {
        let (per, rows) = grid_shape(1600.0, 150.0, 8.0, 10_000);
        assert_eq!(per, 10);
        assert_eq!(rows, 1_000);
    }
}
