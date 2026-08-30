//! 인기 코딩 폰트 클릭 다운로드/설치.
//!
//! 미설치 폰트를 GitHub에서 받아 `%LOCALAPPDATA%\nabi\fonts`에 저장한다(시스템 설치
//! 불필요 — 앱이 경로로 직접 로드, 폰트 목록 스캔도 이 폴더를 포함). 직접 .ttf URL이거나
//! 릴리스 zip(PowerShell Expand-Archive로 해제)인 두 경로를 지원한다.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// 다운로드 출처.
pub(crate) enum Source {
    /// 직접 .ttf URL.
    Ttf(&'static str),
    /// 저장소 최신 릴리스의 zip → 내부에서 `patt` 포함(굵게/기울임 제외) ttf 추출.
    Zip { repo: &'static str, patt: &'static str },
}

/// 설치 가능한 폰트(표시명 = 패밀리명).
pub(crate) struct FontDef {
    pub label: &'static str,
    pub source: Source,
}

/// 제공 폰트 목록(요청: D2Coding, IBM Plex Mono, Cascadia Code, JetBrains Mono).
pub(crate) fn catalog() -> Vec<FontDef> {
    vec![
        FontDef {
            label: "JetBrains Mono",
            source: Source::Ttf(
                "https://raw.githubusercontent.com/JetBrains/JetBrainsMono/master/fonts/ttf/JetBrainsMono-Regular.ttf",
            ),
        },
        FontDef {
            label: "IBM Plex Mono",
            source: Source::Ttf(
                "https://raw.githubusercontent.com/IBM/plex/master/packages/plex-mono/fonts/complete/ttf/IBMPlexMono-Regular.ttf",
            ),
        },
        FontDef { label: "Cascadia Code", source: Source::Zip { repo: "microsoft/cascadia-code", patt: "cascadiacode" } },
        FontDef { label: "D2Coding", source: Source::Zip { repo: "naver/d2codingfont", patt: "d2coding" } },
    ]
}

/// 다운로드 폰트를 두는 폴더(폰트 목록 스캔도 포함).
pub(crate) fn fonts_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|l| PathBuf::from(l).join(r"nabi\fonts"))
}

/// 진행 상태(UI가 폴링해 완료 시 폰트를 적용한다).
#[derive(Clone)]
pub(crate) enum InstState {
    Working,
    Done(String),
    Error(String),
}

/// 백그라운드 다운로드와 UI가 공유하는 설치 상태(폰트 라벨 → 상태).
#[derive(Clone, Default)]
pub(crate) struct FontInstaller {
    state: Arc<Mutex<HashMap<String, InstState>>>,
}

impl FontInstaller {
    pub(crate) fn status(&self, label: &str) -> Option<InstState> {
        self.state.lock().ok()?.get(label).cloned()
    }

    pub(crate) fn clear(&self, label: &str) {
        if let Ok(mut m) = self.state.lock() {
            m.remove(label);
        }
    }

    fn set(&self, label: &str, s: InstState) {
        if let Ok(mut m) = self.state.lock() {
            m.insert(label.to_string(), s);
        }
    }

    /// 라벨에 해당하는 폰트를 백그라운드로 받아 설치한다(완료 시 repaint 요청).
    pub(crate) fn install_async(&self, label: &'static str, ctx: &egui::Context) {
        self.set(label, InstState::Working);
        let me = self.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let r = catalog()
                .iter()
                .find(|d| d.label == label)
                .ok_or_else(|| "알 수 없는 폰트".to_string())
                .and_then(install);
            me.set(label, match r {
                Ok(p) => InstState::Done(p),
                Err(e) => InstState::Error(e),
            });
            ctx.request_repaint();
        });
    }
}

/// 폰트를 받아 nabi 폰트 폴더에 저장하고 그 경로를 돌려준다.
fn install(def: &FontDef) -> Result<String, String> {
    let dir = fonts_dir().ok_or("폰트 폴더 경로 없음")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join(format!("{}.ttf", def.label.replace(' ', ""))).to_string_lossy().into_owned();
    match &def.source {
        Source::Ttf(url) => nabi_release::download_file(url, &dest)?,
        Source::Zip { repo, patt } => install_from_zip(repo, patt, &dest)?,
    }
    Ok(dest)
}

/// 최신 릴리스 zip을 받아 내부 ttf를 추출해 dest로 복사한다.
fn install_from_zip(repo: &str, patt: &str, dest: &str) -> Result<(), String> {
    let body = nabi_release::http_get_text("api.github.com", &format!("/repos/{repo}/releases/latest"))?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("JSON 파싱: {e}"))?;
    let zip_url = json["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["name"].as_str().unwrap_or("").to_lowercase().ends_with(".zip")))
        .and_then(|a| a["browser_download_url"].as_str())
        .ok_or("릴리스에 zip 자산이 없습니다")?
        .to_string();
    let tmp = std::env::temp_dir();
    let zip = tmp.join("nabi-font.zip");
    let out = tmp.join("nabi-font-extract");
    nabi_release::download_file(&zip_url, &zip.to_string_lossy())?;
    extract_zip(&zip, &out)?;
    let ttf = find_ttf(&out, patt).ok_or("zip에서 ttf를 찾지 못했습니다")?;
    std::fs::copy(&ttf, dest).map_err(|e| format!("복사 실패: {e}"))?;
    Ok(())
}

/// PowerShell Expand-Archive로 zip을 해제한다(창 숨김).
fn extract_zip(zip: &Path, out: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(out).ok();
    use std::os::windows::process::CommandExt;
    let cmd = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        zip.display(),
        out.display()
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .status()
        .map_err(|e| format!("압축 해제 실행 실패: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("압축 해제 실패".into())
    }
}

/// 굵게/기울임 등 변형을 뜻하는 이름 키워드(Regular를 고르기 위해 제외).
fn is_variant(lower: &str) -> bool {
    ["bold", "italic", "light", "semibold", "thin", "medium", "extra", "ligature", "oblique"]
        .iter()
        .any(|k| lower.contains(k))
}

/// out 아래를 재귀로 훑어 patt를 포함하는 Regular ttf를 찾는다(변형 제외, regular 우선).
fn find_ttf(out: &Path, patt: &str) -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    let mut stack = vec![out.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            // 링크를 따라가면 끝없이 돈다 — 내려받아 푼 글꼴 묶음 안에 링크가 있을 수 있다.
            if nabi_fs::walk::is_real_dir(&p) {
                stack.push(p);
                continue;
            }
            let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            if name.ends_with(".ttf") && name.contains(patt) && !is_variant(&name) {
                cands.push(p);
            }
        }
    }
    cands.sort_by_key(|p| p.to_string_lossy().len()); // 짧은 경로(기본 Regular)를 우선.
    cands
        .iter()
        .find(|p| p.to_string_lossy().to_lowercase().contains("regular"))
        .or_else(|| cands.first())
        .cloned()
}
