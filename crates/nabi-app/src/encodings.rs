//! nabiPad 인코딩 목록(단일 진실원) — 유사 인코딩끼리 그룹으로 묶어 그룹 구분선과 함께
//! 드롭다운에 보여 준다. 라벨은 모두 encoding_rs(WHATWG)가 인식하는 값이라 재디코드에 그대로 쓴다.

/// (그룹 이름, 그 그룹의 인코딩 라벨들). 드롭다운은 그룹마다 헤더 + 구분선으로 표시한다.
pub(crate) const ENCODING_GROUPS: &[(&str, &[&str])] = &[
    ("Unicode", &["UTF-8", "UTF-16LE", "UTF-16BE"]),
    ("한국어 (Korean)", &["EUC-KR"]),
    ("일본어 (Japanese)", &["Shift_JIS", "EUC-JP", "ISO-2022-JP"]),
    ("중국어 (Chinese)", &["GBK", "gb18030", "Big5"]),
    ("서유럽 (Western)", &["windows-1252", "ISO-8859-15"]),
    ("중부유럽 (Central)", &["windows-1250", "ISO-8859-2"]),
    ("키릴 (Cyrillic)", &["windows-1251", "KOI8-R", "ISO-8859-5"]),
    ("기타 (Other)", &["windows-1254", "windows-1255", "windows-1256", "windows-1257"]),
];

/// 그룹 헤더(비클릭) + 구분선과 함께 인코딩을 나열하는 드롭다운 본문.
/// 항목 클릭 시 그 라벨을 반환한다(재디코드 트리거).
pub(crate) fn encoding_menu(ui: &mut egui::Ui, current: &str) -> Option<String> {
    let mut picked = None;
    // 작은 화면/창에서도 전체 인코딩을 고를 수 있도록 스크롤 영역으로 감싼다(목록이 길어 잘릴 수 있음).
    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
        for (gi, (group, labels)) in ENCODING_GROUPS.iter().enumerate() {
            if gi > 0 {
                ui.separator();
            }
            ui.label(egui::RichText::new(*group).weak().small());
            for &e in *labels {
                if ui.selectable_label(current == e, e).clicked() {
                    picked = Some(e.to_string());
                    ui.close_menu();
                }
            }
        }
    });
    picked
}
