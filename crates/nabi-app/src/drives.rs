//! "내 컴퓨터" 드라이브 목록 + 디스크 용량(Win32 직접 링크 — windows feature 불필요).
//! 파일 브라우저에서 b.path가 비면 이 뷰를 그리고, 드라이브 안에서는 용량 라벨을 보여준다.

use crate::browserfs::human;
use nabi_i18n::{tr, Lang};
use std::path::{Path, PathBuf};

/// 한 드라이브의 용량(바이트).
pub(crate) struct DiskSpace {
    pub total: u64,
    pub free: u64,
}
impl DiskSpace {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }
}

/// 사용 가능한 드라이브 루트 목록(["C:\\", "D:\\", …]).
pub(crate) fn drive_roots() -> Vec<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetLogicalDrives() -> u32;
    }
    // SAFETY: 인자 없는 Win32 호출. 비트마스크 한 개를 돌려줄 뿐 포인터를 다루지 않는다.
    let mask = unsafe { GetLogicalDrives() };
    (0u8..26)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:\\", (b'A' + i) as char))
        .collect()
}

/// 경로(드라이브 루트/하위)의 디스크 용량(총·여유). 실패 시 None.
pub(crate) fn disk_space(path: &str) -> Option<DiskSpace> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: wide는 NUL로 끝나는 UTF-16 버퍼이고 호출 동안 살아 있다. 나머지 세 인자는
    // 스택 지역 u64의 가변 참조라 정렬·크기가 맞고 겹치지 않는다.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, &mut total, &mut free) };
    (ok != 0 && total > 0).then_some(DiskSpace { total, free })
}

/// 경로의 드라이브 루트("C:\\")를 추출(드라이브 문자 경로일 때만).
fn drive_root_of(path: &Path) -> Option<String> {
    let s = path.to_string_lossy();
    let b = s.as_bytes();
    (b.len() >= 2 && b[1] == b':').then(|| format!("{}:\\", (b[0] as char).to_ascii_uppercase()))
}

/// 현재 폴더가 속한 드라이브의 용량 라벨(상태 요약줄용). 드라이브 경로가 아니면 None.
pub(crate) fn drive_usage_label(path: &Path, lang: Lang) -> Option<String> {
    let root = drive_root_of(path)?;
    let d = disk_space(&root)?;
    Some(format!(
        "\u{1f4bd} {} {} / {} ({} {})",
        root,
        human(d.used()),
        human(d.total),
        tr(lang, "browser.free"),
        human(d.free)
    ))
}

/// "내 컴퓨터" 뷰: 각 드라이브를 사용량 막대와 함께 그린다. 클릭한 드라이브 경로를 돌려준다.
pub(crate) fn drives_view(ui: &mut egui::Ui, lang: Lang) -> Option<PathBuf> {
    let mut nav = None;
    ui.add_space(4.0);
    ui.heading(format!("\u{1f5a5} {}", tr(lang, "browser.mycomputer")));
    ui.add_space(4.0);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for root in drive_roots() {
            ui.horizontal(|ui| {
                if ui.selectable_label(false, format!("\u{1f4bd} {root}")).clicked() {
                    nav = Some(PathBuf::from(&root));
                }
                if let Some(d) = disk_space(&root) {
                    let frac = d.used() as f32 / d.total as f32;
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .desired_width(170.0)
                            .text(format!("{} / {}", human(d.used()), human(d.total))),
                    );
                    ui.label(format!("{} {}", tr(lang, "browser.free"), human(d.free)));
                }
            });
        }
    });
    nav
}
