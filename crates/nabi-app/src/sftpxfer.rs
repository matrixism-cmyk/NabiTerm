//! SFTP 전송 큐 — 항목 모델 + 렌더(다운로드/업로드 진행·완료, 크기 표시).

use crate::app::NabiApp;
use crate::sftppath::join_path;
use nabi_i18n::{tr, Lang};
use nabi_proto::Command;

impl NabiApp {
    /// 원격 항목을 탐색기로 드래그-아웃(파일=가상파일, 폴더=재귀 다운로드). DoDragDrop 블로킹.
    pub(crate) fn start_remote_drag(&self, name: &str, size: u64, is_dir: bool, id: nabi_proto::SftpId) {
        let remote = join_path(&self.sftp.path, name);
        let tx = self.orch.cmd_tx.clone();
        if is_dir {
            crate::windndfolder::drag_out_remote_dir(name.to_string(), remote, id, tx);
        } else {
            let file = crate::windndvirt::RemoteFile { name: name.to_string(), size, remote_path: remote };
            crate::windndvirt::drag_out_remote(file, id, tx);
        }
    }

    /// 로컬 경로를 현재 원격 디렉터리의 하위 폴더(subfolder)로 업로드한다(폴더 행에 드롭).
    pub(crate) fn upload_into(&mut self, subfolder: &str, local: std::path::PathBuf) {
        let Some(id) = self.sftp.id else { return };
        let Some(fname) = local.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        let remote = join_path(&join_path(&self.sftp.path, subfolder), &fname);
        let size = std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
        let is_dir = local.is_dir();
        let local = local.to_string_lossy().into_owned();
        let cmd = if is_dir {
            Command::SftpUploadDir { id, local, remote }
        } else {
            Command::SftpUpload { id, local, remote }
        };
        self.push_xfer(format!("{subfolder}/{fname}"), true, size, cmd);
    }

    /// 로컬 경로 하나를 현재 원격 디렉터리로 업로드한다(파일/폴더 자동, 앱 내 DnD·드롭 공용).
    /// OS 클립보드(CF_HDROP)의 로컬 파일들을 현재 SFTP 폴더로 업로드(붙여넣기, FileZilla식).
    pub(crate) fn sftp_paste_upload(&mut self) {
        if self.sftp.id.is_none() {
            return;
        }
        for src in crate::winclip::paste_paths() {
            self.upload_local_path(src);
        }
    }

    pub(crate) fn upload_local_path(&mut self, local: std::path::PathBuf) {
        let Some(id) = self.sftp.id else { return };
        let Some(fname) = local.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return;
        };
        let remote = join_path(&self.sftp.path, &fname);
        let is_dir = local.is_dir();
        let size = std::fs::metadata(&local).map(|m| m.len()).unwrap_or(0);
        let local = local.to_string_lossy().into_owned();
        let cmd = if is_dir {
            Command::SftpUploadDir { id, local, remote }
        } else {
            Command::SftpUpload { id, local, remote }
        };
        self.push_xfer(fname, true, size, cmd);
    }
}

/// 한 건의 파일 전송 상태.
pub(crate) struct Transfer {
    pub name: String,
    pub up: bool,
    pub size: u64,
    pub bytes: u64,
    pub done: bool,
    pub ok: bool,
    /// 전송 시작 시각(속도 계산용).
    pub started: std::time::Instant,
    /// 재시도용 원본 명령(실패 시 재전송, H2). 다운로드 resume는 재시도 시 그대로 이어받기.
    pub retry: Option<Command>,
    /// 실패 사유(항목별 오류 툴팁, 성공 시 빈 문자열).
    pub err: String,
}

impl Transfer {
    pub(crate) fn new(name: String, up: bool, size: u64) -> Self {
        Self { name, up, size, bytes: 0, done: false, ok: false, started: std::time::Instant::now(), retry: None, err: String::new() }
    }
}

impl NabiApp {
    /// 전송 명령을 보내고 큐에 항목을 추가한다(재시도용 명령 보관, H2). 모든 업/다운로드 경로 공용.
    pub(crate) fn push_xfer(&mut self, name: String, up: bool, size: u64, cmd: Command) {
        self.orch.send(cmd.clone());
        let mut t = Transfer::new(name, up, size);
        t.retry = Some(cmd);
        self.sftp.transfers.push(t);
    }
}

/// 전송 큐 집계(순수, H1) — (bytes, size, speed) 튜플들을 합산해 (총bytes, 총size, 총speed B/s).
/// FileZilla식 큐 헤더(전체 진행률·합산 속도)에 쓴다. 시간 의존을 분리해 단위테스트가 가능하다.
pub(crate) fn xfer_totals(items: &[(u64, u64, u64)]) -> (u64, u64, u64) {
    items.iter().fold((0, 0, 0), |(b, s, sp), &(tb, ts, tsp)| (b + tb, s + ts, sp + tsp))
}

/// 현재 속도로 남은 바이트를 보내는 데 걸릴 예상 초(시작 직후·완료·정보부족이면 None).
pub(crate) fn eta_secs(bytes: u64, size: u64, elapsed: f64) -> Option<u64> {
    if bytes == 0 || elapsed <= 0.0 || bytes >= size {
        return None;
    }
    let speed = bytes as f64 / elapsed;
    (speed > 0.0).then(|| ((size - bytes) as f64 / speed) as u64)
}

/// 초를 사람이 읽는 짧은 표기로(예: 45s, 1m23s, 2h05m).
pub(crate) fn human_secs(s: u64) -> String {
    if s >= 3600 {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

/// 전송 큐를 그린다. 반환 (clear, cancel): 🗑=완료 정리, ⏹=진행 중 전송 취소.
pub(crate) fn show_transfers(ui: &mut egui::Ui, transfers: &[Transfer], lang: Lang) -> (bool, bool, Option<usize>) {
    if transfers.is_empty() {
        return (false, false, None);
    }
    let mut clear = false;
    let mut cancel = false;
    let mut retry = None;
    ui.separator();
    let done = transfers.iter().filter(|t| t.done).count();
    let active = transfers.iter().any(|t| !t.done);
    // 진행 중 전송의 전체 진행률·합산 속도(FileZilla식 큐 요약, H1).
    let summary = if active {
        let items: Vec<(u64, u64, u64)> = transfers.iter().filter(|t| !t.done).map(|t| {
            let s = t.started.elapsed().as_secs_f64();
            let sp = if s > 0.3 && t.bytes > 0 { (t.bytes as f64 / s) as u64 } else { 0 };
            (t.bytes, t.size, sp)
        }).collect();
        let (b, sz, sp) = xfer_totals(&items);
        let pct = (b * 100).checked_div(sz).unwrap_or(0);
        // 합산 ETA(남은 총 시간) — 큐 속도가 있을 때만(FileZilla식).
        let eta = if sp > 0 && sz > b { format!(" \u{00b7} ~{}", human_secs((sz - b) / sp)) } else { String::new() };
        format!("  \u{2211}{pct}% \u{00b7} {}/s{eta}", crate::browserfs::human(sp))
    } else {
        String::new()
    };
    ui.horizontal(|ui| {
        ui.label(format!("{} {done}/{}{summary}", tr(lang, "sftp.transfers"), transfers.len()));
        if ui.small_button("\u{1f5d1}").clicked() {
            clear = true;
        }
        if active
            && ui
                .small_button("\u{23f9}")
                .on_hover_text(tr(lang, "sftp.cancel"))
                .clicked()
        {
            cancel = true;
        }
    });
    for (i, t) in transfers.iter().enumerate() {
        let dir = if t.up { "\u{2b06}" } else { "\u{2b07}" };
        if !t.done && t.size > 0 {
            // 진행 중: 퍼센트 막대(이름 + 전송 속도 오버레이).
            let frac = (t.bytes as f32 / t.size as f32).clamp(0.0, 1.0);
            let secs = t.started.elapsed().as_secs_f64();
            let txt = if secs > 0.3 && t.bytes > 0 {
                let sp = (t.bytes as f64 / secs) as u64;
                let speed = crate::browserfs::human(sp);
                match eta_secs(t.bytes, t.size, secs) {
                    Some(e) => format!("{dir} {} ({speed}/s \u{00b7} ~{})", t.name, human_secs(e)),
                    None => format!("{dir} {} ({speed}/s)", t.name),
                }
            } else {
                format!("{dir} {}", t.name)
            };
            ui.add(egui::ProgressBar::new(frac).desired_width(190.0).text(txt));
        } else {
            let st = if t.ok { "\u{2713}" } else if t.done { "\u{2717}" } else { "\u{23f3}" };
            let failed = t.done && !t.ok && t.retry.is_some();
            ui.horizontal(|ui| {
                let lbl = ui.label(format!("{dir} {} {st}  {}", t.name, crate::browserfs::human(t.size)));
                // 실패 항목은 사유를 툴팁으로(WinSCP식 항목별 오류).
                if failed && !t.err.is_empty() {
                    lbl.on_hover_text(&t.err);
                }
                // 실패 전송은 ↻로 재시도(H2, 원본 명령 재전송).
                if failed && ui.small_button("\u{21bb}").on_hover_text(tr(lang, "sftp.retry")).clicked() {
                    retry = Some(i);
                }
            });
        }
    }
    (clear, cancel, retry)
}

#[cfg(test)]
mod tests {
    use super::{eta_secs, human_secs, xfer_totals};

    #[test]
    fn totals_sum_components() {
        let items = [(10u64, 100u64, 5u64), (40, 100, 15), (0, 50, 0)];
        assert_eq!(xfer_totals(&items), (50, 250, 20)); // bytes·size·speed 각각 합산.
        assert_eq!(xfer_totals(&[]), (0, 0, 0));
    }


    #[test]
    fn eta_and_format() {
        // 1초에 500/1000 → 남은 500, 속도 500/s → 약 1초.
        assert_eq!(eta_secs(500, 1000, 1.0), Some(1));
        assert_eq!(eta_secs(0, 1000, 1.0), None); // 시작 직후.
        assert_eq!(eta_secs(1000, 1000, 1.0), None); // 완료.
        assert_eq!(human_secs(45), "45s");
        assert_eq!(human_secs(83), "1m23s");
        assert_eq!(human_secs(7505), "2h05m");
    }
}
