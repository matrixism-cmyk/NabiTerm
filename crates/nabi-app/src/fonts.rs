//! CJK 폰트 폴백 설치.
//!
//! 기본 egui 폰트는 한글/일본어 글리프가 없으므로, Windows 시스템 CJK 폰트를
//! Monospace/Proportional 패밀리의 폴백으로 추가한다(라틴은 기존 등폭 유지,
//! CJK는 폴백으로 렌더).

/// 설치된 등폭(monospace) 글꼴 (패밀리명, 전체경로) 목록. 시스템·사용자 Fonts 폴더 스캔.
/// 폰트 파일을 파싱해 등폭 여부·패밀리명을 얻는다(.ttc는 egui가 단일 face 로드를 못 해 제외).
/// 결과는 캐시(매 프레임 호출 — 첫 호출에만 전체 파싱).
static FONT_CACHE: std::sync::Mutex<Option<Vec<(String, String)>>> = std::sync::Mutex::new(None);

pub(crate) fn list_monospace_fonts() -> Vec<(String, String)> {
    let mut c = FONT_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if c.is_none() {
        *c = Some(scan_monospace_fonts());
    }
    c.clone().unwrap_or_default()
}

/// 새 폰트를 받은 뒤 호출 — 다음 목록 조회에서 재스캔(받은 폰트 반영).
pub(crate) fn invalidate_font_cache() {
    *FONT_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// 이름이 명백히 등폭 계열인지(post 테이블의 isFixedPitch를 안 켠 코딩 폰트 보강).
///
/// `"cod"`인 이유: `"code"`로 두면 **D2Coding·나눔고딕코딩이 빠진다**(둘 다 `coding`이라
/// `code`를 담지 않는다). 국내에서 가장 많이 쓰는 코딩 글꼴 둘이 목록에 안 뜨고 있었다
/// (2026-08-27, 파서를 옮기며 붙인 시험이 잡았다).
fn name_is_mono(lower: &str) -> bool {
    ["mono", "cod", "console", "consol", "courier", "terminal", "fixed", "typewriter"]
        .iter()
        .any(|k| lower.contains(k))
}

/// 폰트 파일에서 (패밀리명, 등폭 여부)를 파싱한다(실패 시 None).
///
/// `skrifa`를 쓴다 — 예전에는 `ttf-parser`였는데 관리가 중단됐다(RUSTSEC-2026-0192,
/// 안전한 상위 판 없음). skrifa는 **이미 트리에 있다**(egui의 글꼴 스택이 쓴다).
/// 옮기면서 의존성이 하나 줄었다.
fn font_meta(path: &std::path::Path) -> Option<(String, bool)> {
    use skrifa::MetadataProvider;
    let data = std::fs::read(path).ok()?;
    let face = skrifa::FontRef::new(&data).ok()?;
    let name = face
        .localized_strings(skrifa::string::StringId::FAMILY_NAME)
        .english_or_first()
        .map(|n| n.chars().collect::<String>())
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    // post 테이블의 isFixedPitch(0이 아니면 등폭). 없는 폰트도 있어 그때는 false —
    // 이름 규칙(name_is_mono)이 그 자리를 메운다.
    // skrifa가 재수출하는 판을 쓴다 — read-fonts 를 따로 걸면 판이 갈려 트레잇이 안 맞는다.
    let mono = skrifa::raw::TableProvider::post(&face)
        .map(|p| p.is_fixed_pitch() != 0)
        .unwrap_or(false);
    Some((name, mono))
}

/// Fonts 폴더의 .ttf/.otf를 파싱해 등폭 폰트만 (패밀리명, 경로)로 수집(패밀리명 중복 제거).
fn scan_monospace_fonts() -> Vec<(String, String)> {
    let dirs = [
        std::env::var_os("WINDIR").map(|w| std::path::PathBuf::from(w).join("Fonts")),
        std::env::var_os("LOCALAPPDATA")
            .map(|l| std::path::PathBuf::from(l).join(r"Microsoft\Windows\Fonts")),
        crate::fontinstall::fonts_dir(), // 앱이 받은 코딩 폰트(D2Coding 등).
    ];
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs.into_iter().flatten() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let fname = e.file_name().to_string_lossy().to_lowercase();
            if !(fname.ends_with(".ttf") || fname.ends_with(".otf")) {
                continue;
            }
            let path = e.path();
            if let Some((name, mono)) = font_meta(&path) {
                if (mono || name_is_mono(&name.to_lowercase())) && seen.insert(name.to_lowercase()) {
                    out.push((name, path.to_string_lossy().into_owned()));
                }
            }
        }
    }
    out.sort();
    out
}

/// CJK 폴백 + (지정 시) 사용자 글꼴 파일을 egui에 설치한다.
/// `user_font`가 존재하는 .ttf/.otf 경로면 Monospace 최우선으로 로드한다.
pub fn install_cjk_fonts(ctx: &egui::Context, user_font: &str) {
    let mut fonts = egui::FontDefinitions::default();

    // 단일 .ttf만 사용(.ttc 컬렉션은 로딩 위험이 있어 제외).
    // - malgun: 한국어 CJK 폴백.
    // - seguisym(Segoe UI Symbol): UI 아이콘 글리프(✎편집·⎘복제·✕삭제·🖧SFTP 등).
    //   egui 기본 이모지 폰트는 이런 기호 상당수가 빠져 두부(□)로 나오므로 폴백 추가.
    let candidates = [
        ("malgun", r"C:\Windows\Fonts\malgun.ttf"),
        ("seguisym", r"C:\Windows\Fonts\seguisym.ttf"),
    ];

    let mut added = Vec::new();
    for (name, path) in candidates {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            added.push(name.to_owned());
        }
    }

    for name in &added {
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts.families.entry(family).or_default().push(name.clone());
        }
    }

    // 사용자 글꼴(파일 경로면 로드해 Monospace 최우선 — 터미널 본문 글꼴).
    let mut user_loaded = false;
    if !user_font.is_empty() && std::path::Path::new(user_font).is_file() {
        if let Ok(data) = std::fs::read(user_font) {
            fonts
                .font_data
                .insert("user_mono".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .insert(0, "user_mono".to_owned());
            user_loaded = true;
        }
    }

    if !added.is_empty() || user_loaded {
        ctx.set_fonts(fonts);
    }
}

#[cfg(test)]
mod tests {
    /// 이름만으로 등폭을 알아보는 보강 규칙(플래그를 안 켠 코딩 폰트용).
    #[test]
    fn coding_font_names_are_recognised() {
        // D2Coding·나눔고딕코딩은 국내에서 가장 많이 쓰는 코딩 글꼴이다 —
        // `"code"` 로는 못 잡았다(둘 다 `coding`).
        for n in ["cascadia mono", "d2coding", "nanumgothiccoding", "consolas", "courier new", "fixedsys"] {
            assert!(super::name_is_mono(n), "{n}");
        }
        for n in ["malgun gothic", "arial", "batang"] {
            assert!(!super::name_is_mono(n), "{n}");
        }
    }

    /// **이 PC의 진짜 글꼴을 읽는다** — 파서를 ttf-parser에서 skrifa로 옮겼으므로,
    /// 컴파일이 되는 것과 실제로 이름·등폭을 읽어 내는 것은 다르다.
    /// 윈도우라면 Consolas가 거의 늘 있다(없으면 시험을 건너뛴다 — 남의 PC를 단정하지 않는다).
    #[test]
    fn a_real_font_file_still_parses() {
        let Some(win) = std::env::var_os("WINDIR") else { return };
        let f = std::path::PathBuf::from(win).join("Fonts").join("consola.ttf");
        if !f.is_file() {
            return;
        }
        let got = super::font_meta(&f).expect("consola.ttf 를 읽지 못했다");
        assert!(got.0.to_lowercase().contains("consolas"), "패밀리명: {}", got.0);
        assert!(got.1, "Consolas 를 등폭으로 보지 못했다");
    }

    /// 등폭이 아닌 글꼴은 등폭이라고 하지 않는다(둘 다 맞아야 목록이 쓸모 있다).
    #[test]
    fn a_proportional_font_is_not_called_monospace() {
        let Some(win) = std::env::var_os("WINDIR") else { return };
        let f = std::path::PathBuf::from(win).join("Fonts").join("arial.ttf");
        if !f.is_file() {
            return;
        }
        let got = super::font_meta(&f).expect("arial.ttf 를 읽지 못했다");
        assert!(!got.1, "Arial 을 등폭이라고 했다");
    }
}
