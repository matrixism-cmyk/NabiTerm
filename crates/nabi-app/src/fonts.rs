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

/// 이름이 명백히 등폭 계열인지(is_monospaced 플래그를 안 켠 코딩 폰트 보강).
fn name_is_mono(lower: &str) -> bool {
    ["mono", "code", "console", "consol", "courier", "terminal", "fixed", "typewriter"]
        .iter()
        .any(|k| lower.contains(k))
}

/// 폰트 파일에서 (패밀리명, 등폭 여부)를 파싱한다(실패 시 None).
fn font_meta(path: &std::path::Path) -> Option<(String, bool)> {
    let data = std::fs::read(path).ok()?;
    let face = ttf_parser::Face::parse(&data, 0).ok()?;
    let name = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
        .and_then(|n| n.to_string())
        .or_else(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))?;
    Some((name, face.is_monospaced()))
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
                .insert(name.to_owned(), egui::FontData::from_owned(data));
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
                .insert("user_mono".to_owned(), egui::FontData::from_owned(data));
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
