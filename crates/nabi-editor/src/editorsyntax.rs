//! nabiPad 구문 강조 자산(syntect) 외부 로드 — 사용자 폴더의 `.sublime-syntax`/`.tmTheme`를
//! 기본 세트에 더해 빌드하고, 바이너리 캐시(packdump)로 빠르게 시작한다(bat 방식).
//!
//! 전역 상태로 두는 이유: egui FrameCache(editorhl)의 Computer는 Default로 생성돼 앱 상태를
//! 주입하기 어렵다. 테마 변경은 캐시 키(테마 이름)로 무효화된다.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use syntect::dumps::{dump_to_uncompressed_file, from_uncompressed_dump_file};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct Assets {
    pub ps: SyntaxSet,
    pub themes: ThemeSet,
}

struct Dirs {
    syntaxes: PathBuf,
    themes: PathBuf,
    cache: PathBuf,
}

static DIRS: OnceLock<Dirs> = OnceLock::new();
static ASSETS: OnceLock<RwLock<Assets>> = OnceLock::new();
static EXT_MAP: OnceLock<RwLock<BTreeMap<String, String>>> = OnceLock::new();
static THEME: OnceLock<RwLock<String>> = OnceLock::new();

const DEFAULT_THEME: &str = "base16-mocha.dark";

/// 앱 시작 시 1회 초기화: 폴더 등록 + 테마/확장자 매핑 적용.
pub fn init(base: &Path, theme: String, ext_map: BTreeMap<String, String>) {
    set_dirs(base);
    set_theme(theme);
    set_ext_map(ext_map);
}

/// nabiPad 설정 페이지의 구문 강조 행(테마 드롭다운 + 외부 폴더/캐시) — settingsui에서 호출.
pub fn settings_ui(ui: &mut egui::Ui, e: &mut nabi_config::EditorConfig, lang: nabi_i18n::Lang) {
    use nabi_i18n::tr;
    ui.label(tr(lang, "nabipad.theme"));
    egui::ComboBox::from_id_salt("nabipad_theme").selected_text(&e.theme).show_ui(ui, |ui| {
        for t in theme_names() {
            if ui.selectable_value(&mut e.theme, t.clone(), &t).clicked() {
                set_theme(e.theme.clone());
            }
        }
    });
    ui.end_row();
    ui.label(tr(lang, "nabipad.syntax.ext"));
    ui.horizontal(|ui| {
        if ui.button(tr(lang, "nabipad.syntax.folder")).clicked() {
            open_dir(syntax_dir());
        }
        if ui.button(tr(lang, "nabipad.theme.folder")).clicked() {
            open_dir(theme_dir());
        }
        if ui.button(tr(lang, "nabipad.syntax.rebuild")).clicked() {
            rebuild();
            set_theme(e.theme.clone());
        }
    });
    ui.end_row();
}

/// 폴더를 만들고 탐색기로 연다(없으면 생성).
fn open_dir(dir: Option<PathBuf>) {
    if let Some(d) = dir {
        let _ = std::fs::create_dir_all(&d);
        let _ = std::process::Command::new("explorer").arg(d).spawn();
    }
}

/// 앱 시작 시 1회: nabiPad 설정 base/nabipad 하위에 구문/테마/캐시 폴더를 등록한다.
pub fn set_dirs(base: &Path) {
    let root = base.join("nabipad");
    let _ = DIRS.set(Dirs {
        syntaxes: root.join("syntaxes"),
        themes: root.join("themes"),
        cache: root.join("cache"),
    });
}

/// 확장자→구문 강제 매핑 설정/갱신(설정 변경 시 호출).
pub fn set_ext_map(map: BTreeMap<String, String>) {
    let lock = EXT_MAP.get_or_init(|| RwLock::new(BTreeMap::new()));
    if let Ok(mut m) = lock.write() {
        *m = map;
    }
}

/// 현재 테마 이름 설정(설정/드롭다운). 캐시 키에 반영돼 즉시 재계산된다.
pub fn set_theme(name: String) {
    let lock = THEME.get_or_init(|| RwLock::new(DEFAULT_THEME.to_string()));
    if let Ok(mut t) = lock.write() {
        *t = name;
    }
}

/// 현재 테마 이름(없으면 기본).
pub fn current_theme() -> String {
    THEME
        .get()
        .and_then(|l| l.read().ok().map(|t| t.clone()))
        .unwrap_or_else(|| DEFAULT_THEME.to_string())
}

fn cache_file() -> Option<PathBuf> {
    DIRS.get().map(|d| d.cache.join("syntaxes.packdump"))
}

fn build_syntaxes() -> SyntaxSet {
    let mut b = SyntaxSet::load_defaults_newlines().into_builder();
    if let Some(d) = DIRS.get() {
        if d.syntaxes.is_dir() {
            let _ = b.add_from_folder(&d.syntaxes, true); // 잘못된 파일 1개로 전체를 막지 않게 실패 무시.
        }
    }
    b.build()
}

fn build_themes() -> ThemeSet {
    let mut t = ThemeSet::load_defaults();
    if let Some(d) = DIRS.get() {
        if d.themes.is_dir() {
            let _ = t.add_from_folder(&d.themes);
        }
    }
    t
}

fn save_cache(ps: &SyntaxSet) {
    if let Some(f) = cache_file() {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = dump_to_uncompressed_file(ps, f);
    }
}

fn load_assets() -> Assets {
    // 캐시가 있으면 거기서(빠름), 없으면 폴더 빌드 후 캐시 저장.
    let ps = cache_file()
        .filter(|p| p.exists())
        .and_then(|p| from_uncompressed_dump_file::<SyntaxSet, _>(&p).ok())
        .unwrap_or_else(|| {
            let ps = build_syntaxes();
            save_cache(&ps);
            ps
        });
    Assets { ps, themes: build_themes() }
}

/// 전역 자산(없으면 1회 빌드). 읽기 잠금으로 접근.
pub fn assets() -> &'static RwLock<Assets> {
    ASSETS.get_or_init(|| RwLock::new(load_assets()))
}

/// 사용자 폴더에서 자산을 다시 빌드하고 캐시를 갱신한다("캐시 다시 빌드" 명령).
pub fn rebuild() {
    let assets = Assets { ps: build_syntaxes(), themes: build_themes() };
    save_cache(&assets.ps);
    match ASSETS.get() {
        Some(lock) => {
            if let Ok(mut a) = lock.write() {
                *a = assets;
            }
        }
        None => {
            let _ = ASSETS.set(RwLock::new(assets));
        }
    }
}

/// 정렬된 테마 이름 목록(설정 드롭다운용).
pub fn theme_names() -> Vec<String> {
    assets()
        .read()
        .map(|a| {
            let mut v: Vec<String> = a.themes.themes.keys().cloned().collect();
            v.sort();
            v
        })
        .unwrap_or_default()
}

/// 확장자에 강제 매핑된 구문 이름(있으면).
pub fn mapped_syntax(ext: &str) -> Option<String> {
    EXT_MAP.get()?.read().ok()?.get(ext).cloned()
}

/// 폴더 경로(설정 UI "폴더 열기"용).
pub fn syntax_dir() -> Option<PathBuf> {
    DIRS.get().map(|d| d.syntaxes.clone())
}
pub fn theme_dir() -> Option<PathBuf> {
    DIRS.get().map(|d| d.themes.clone())
}

#[cfg(test)]
mod tests {
    /// 번들 기본 구문셋에 흔한 확장자(XML 포함)가 있어야 한다 — 구문강조 동작 보증(보고 대응).
    #[test]
    fn defaults_include_common_syntaxes() {
        let ps = syntect::parsing::SyntaxSet::load_defaults_newlines();
        for ext in ["xml", "json", "rs", "py", "html", "css", "yaml"] {
            assert!(ps.find_syntax_by_extension(ext).is_some(), "기본셋에 .{ext} 구문 없음");
        }
    }
}
