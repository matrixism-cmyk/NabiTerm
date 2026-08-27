//! nabiPad 인코딩 목록(단일 진실원) — 유사 인코딩끼리 그룹으로 묶어 그룹 구분선과 함께
//! 드롭다운에 보여 준다. 라벨은 모두 encoding_rs(WHATWG)가 인식하는 값이라 재디코드에 그대로 쓴다.

/// (그룹 이름, 그 그룹의 인코딩 라벨들). 드롭다운은 그룹마다 헤더 + 구분선으로 표시한다.
pub const ENCODING_GROUPS: &[(&str, &[&str])] = &[
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
pub fn encoding_menu(ui: &mut egui::Ui, current: &str) -> Option<String> {
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
                    ui.close();
                }
            }
        }
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::ENCODING_GROUPS;

    /// **드롭다운에 있는 라벨을 전부 실제로 디코드할 수 있는가.**
    ///
    /// 헤더에 "라벨은 모두 encoding_rs(WHATWG)가 인식하는 값"이라고 적어 두었는데 그것을
    /// 확인하는 것이 없었다. 틀린 라벨이 하나 섞이면 사용자는 그것을 고르고도 아무 일이
    /// 일어나지 않는 것을 본다 — `decoder_for` 가 모르는 라벨을 **조용히 UTF-8로** 떨어뜨리기
    /// 때문이다. 글자는 깨진 채 그대로고, 왜 안 되는지 알 방법도 없다.
    #[test]
    fn every_offered_label_is_a_real_encoding() {
        for (group, labels) in ENCODING_GROUPS {
            for label in *labels {
                let got = encoding_rs::Encoding::for_label(label.as_bytes());
                assert!(got.is_some(), "[{group}] 의 {label} 은 encoding_rs 가 모르는 이름이다");
            }
        }
    }

    /// 같은 인코딩이 두 그룹에 겹쳐 있지 않은가 — 드롭다운에 두 번 나오면 고르는 사람이 헷갈린다.
    #[test]
    fn no_label_appears_twice() {
        let mut seen = std::collections::HashSet::new();
        for (_, labels) in ENCODING_GROUPS {
            for label in *labels {
                assert!(seen.insert(*label), "{label} 이 두 번 나온다");
            }
        }
        assert!(seen.len() > 15, "목록이 너무 짧다({}개)", seen.len());
    }

    /// UTF-8 은 반드시 있어야 한다 — 기본값인데 목록에 없으면 되돌릴 방법이 없다.
    #[test]
    fn utf8_is_always_offered() {
        let has = ENCODING_GROUPS.iter().any(|(_, l)| l.contains(&"UTF-8"));
        assert!(has, "UTF-8 이 목록에 없다");
    }
}
