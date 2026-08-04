//! OS 파일 드롭(탐색기 → nabi) 라우팅. winit이 드래그 중 커서 위치를 주지 않으므로,
//! 드롭 순간 실제 커서 위치(GetCursorPos→ScreenToClient)로 어느 패널 위인지 판정한다.
//! 브라우저 탭/사이드바=폴더로 복사, SFTP=원격 업로드, 그 외=터미널에 경로 입력.

use crate::app::NabiApp;
use bytes::Bytes;
use nabi_proto::Command;

/// OS 드롭이 떨어질 수 있는 패널.
#[derive(Clone, PartialEq)]
pub(crate) enum DropTarget {
    BrowserTab(nabi_types::PaneId),
    SidebarBrowser,
    Sftp,
}

impl NabiApp {
    /// 매 프레임 OS 파일 드롭을 적절한 패널로 라우팅한다(central 이후 호출).
    pub(crate) fn dispatch_dropped_files(&mut self, ctx: &egui::Context) {
        let paths: Vec<std::path::PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if paths.is_empty() {
            return;
        }
        ctx.input_mut(|i| i.raw.dropped_files.clear());
        // 드롭 위치(커서) → 이번 프레임 렌더된 드롭 존 중 포함하는 것.
        let ppp = ctx.pixels_per_point();
        let pos = self
            .hwnd
            .and_then(crate::windnd::cursor_client_px)
            .map(|(x, y)| egui::pos2(x as f32 / ppp, y as f32 / ppp));
        let target = pos.and_then(|p| {
            self.drop_zones.iter().find(|(_, r)| r.contains(p)).map(|(t, _)| t.clone())
        });
        match target {
            Some(DropTarget::BrowserTab(pane)) => {
                if let Some(dir) = self.browser_tabs.get(&pane).map(|b| b.path.clone()) {
                    for src in &paths {
                        crate::browserops::copy_into(src, &dir);
                    }
                }
            }
            Some(DropTarget::SidebarBrowser) => {
                let dir = self.browser.path.clone();
                for src in &paths {
                    crate::browserops::copy_into(src, &dir);
                }
            }
            Some(DropTarget::Sftp) => {
                if self.sftp.id.is_some() {
                    for local in paths {
                        self.upload_local_path(local);
                    }
                }
            }
            None => self.drop_to_terminal(&paths),
        }
    }

    /// 패널이 아닌 곳에 드롭 → 포커스 터미널 pane에 경로 입력(공백 시 따옴표).
    fn drop_to_terminal(&mut self, paths: &[std::path::PathBuf]) {
        let Some(pane) = self.focused_pane() else {
            return;
        };
        let strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        let text = format_dropped_paths(&strs);
        self.orch.send(Command::WriteInput { pane, data: Bytes::from(text.into_bytes()) });
    }
}

/// 떨군 경로들을 셸 입력 문자열로 만든다(공백 있으면 따옴표, 공백으로 구분).
fn format_dropped_paths(paths: &[String]) -> String {
    let mut s = String::new();
    for p in paths {
        if p.contains(' ') {
            s.push('"');
            s.push_str(p);
            s.push('"');
        } else {
            s.push_str(p);
        }
        s.push(' ');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces() {
        let out = format_dropped_paths(&["a.txt".into(), "my file.txt".into()]);
        assert_eq!(out, "a.txt \"my file.txt\" ");
    }
}
