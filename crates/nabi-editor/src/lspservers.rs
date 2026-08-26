//! **어떤 파일에 어떤 언어 서버를 붙일까** — 언어·서버·프로젝트 루트 표식을 한 곳에.
//!
//! 지금까지 LSP는 러스트 하나에만 붙어 있었다. 확장자 검사(`== "rs"`)와 서버 이름
//! (`"rust-analyzer"`)이 앱 코드 세 곳에 흩어져 박혀 있었고, 그래서 파이썬 파일을 열면
//! 색은 입혀지는데(트리시터는 다섯 언어를 안다) **정의로는 못 갔다.**
//!
//! 코어(`LspClient::start(server, root)`)는 처음부터 서버 이름을 받도록 돼 있었다.
//! 막고 있던 것은 앱 쪽 하드코딩뿐이라, 표 하나를 두고 거기서 읽게 하면 풀린다.
//!
//! ## 루트를 왜 언어마다 다르게 찾나
//!
//! 언어 서버는 **프로젝트 루트**를 받아야 제 일을 한다. 러스트는 `Cargo.toml`, 파이썬은
//! `pyproject.toml`이나 `.git`, 타입스크립트는 `package.json`이 그 표식이다. 루트를 잘못
//! 주면 서버가 남의 파일을 훑거나 아무것도 못 찾는다.
//!
//! 표식이 하나도 없으면 **파일이 있는 폴더**를 쓴다 — 한 파일만 열어 보는 경우가 흔하고,
//! 그때도 정의로 가기 정도는 되는 편이 낫다.

use std::path::{Path, PathBuf};

/// 한 언어의 서버 설정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LangServer {
    /// 이 서버가 맡는 확장자들.
    pub exts: &'static [&'static str],
    /// 실행할 명령(경로 없이 이름만 — PATH에서 찾는다).
    pub cmd: &'static str,
    /// 명령에 붙일 인자(없으면 빈 배열).
    pub args: &'static [&'static str],
    /// 프로젝트 루트를 알려 주는 파일들(위로 올라가며 찾는다).
    pub markers: &'static [&'static str],
    /// 사람이 읽을 언어 이름(설정 화면·안내용).
    pub label: &'static str,
}

/// 아는 언어 서버들.
///
/// **깔려 있지 않은 것이 기본이다.** 여기 있다고 켜지는 것이 아니라, PATH에 그 명령이
/// 있을 때만 붙는다. 없으면 조용히 지나간다(편집기는 평소대로 동작한다).
pub const SERVERS: &[LangServer] = &[
    LangServer {
        exts: &["rs"],
        cmd: "rust-analyzer",
        args: &[],
        markers: &["Cargo.toml"],
        label: "Rust",
    },
    LangServer {
        exts: &["py", "pyi"],
        cmd: "pyright-langserver",
        args: &["--stdio"],
        markers: &["pyproject.toml", "setup.py", "requirements.txt", ".git"],
        label: "Python",
    },
    LangServer {
        exts: &["ts", "tsx", "js", "jsx", "mjs", "cjs"],
        cmd: "typescript-language-server",
        args: &["--stdio"],
        markers: &["tsconfig.json", "jsconfig.json", "package.json", ".git"],
        label: "TypeScript / JavaScript",
    },
    LangServer {
        exts: &["go"],
        cmd: "gopls",
        args: &[],
        markers: &["go.mod", ".git"],
        label: "Go",
    },
    LangServer {
        exts: &["c", "h", "cpp", "cc", "hpp"],
        cmd: "clangd",
        args: &[],
        markers: &["compile_commands.json", "CMakeLists.txt", ".git"],
        label: "C / C++",
    },
];

/// 이 확장자를 맡는 서버(없으면 None). 대소문자는 보지 않는다.
pub fn for_ext(ext: &str) -> Option<&'static LangServer> {
    let e = ext.to_ascii_lowercase();
    SERVERS.iter().find(|s| s.exts.contains(&e.as_str()))
}

/// 프로젝트 루트를 찾는다 — 표식 파일이 있는 가장 가까운 조상 폴더.
///
/// 표식이 없으면 **파일이 있는 폴더**를 돌려준다. 한 파일만 열어 보는 경우가 흔하고,
/// 그때도 서버가 붙는 편이 낫다(안 붙으면 사용자에게는 그냥 안 되는 것이다).
pub fn project_root(file: &Path, markers: &[&str]) -> Option<PathBuf> {
    // `Path::new("a.rs").parent()`는 None이 아니라 **빈 경로**를 준다. 그대로 두면 언어
    // 서버에 빈 루트를 넘기게 되는데, 그때 서버가 무엇을 훑을지는 서버 마음이다.
    let mut dir = file.parent().filter(|p| !p.as_os_str().is_empty())?;
    let first = dir.to_path_buf();
    loop {
        if markers.iter().any(|m| dir.join(m).exists()) {
            return Some(dir.to_path_buf());
        }
        match dir.parent() {
            Some(p) => dir = p,
            None => return Some(first),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{for_ext, project_root, SERVERS};
    use std::path::Path;

    /// **러스트는 그대로여야 한다** — 되던 것이 깨지면 이 배치는 실패다.
    #[test]
    fn rust_still_maps_to_rust_analyzer() {
        let s = for_ext("rs").expect("러스트가 사라졌다");
        assert_eq!(s.cmd, "rust-analyzer");
        assert_eq!(s.markers, &["Cargo.toml"]);
        assert!(s.args.is_empty(), "예전에는 인자 없이 띄웠다");
    }

    #[test]
    fn other_languages_are_known_now() {
        for (ext, cmd) in [("py", "pyright-langserver"), ("ts", "typescript-language-server"), ("go", "gopls")] {
            assert_eq!(for_ext(ext).map(|s| s.cmd), Some(cmd), "{ext}");
        }
    }

    /// 대소문자를 가리지 않는다(`.RS`로 저장된 파일이 있다).
    #[test]
    fn the_extension_match_ignores_case() {
        assert_eq!(for_ext("RS").map(|s| s.cmd), Some("rust-analyzer"));
        assert_eq!(for_ext("Py").map(|s| s.cmd), Some("pyright-langserver"));
    }

    /// 모르는 확장자는 **없다고 답한다** — 아무 서버나 붙이면 안 된다.
    #[test]
    fn an_unknown_extension_has_no_server() {
        for e in ["txt", "md", "log", ""] {
            assert!(for_ext(e).is_none(), "{e}에 서버가 붙었다");
        }
    }

    /// 한 확장자를 두 서버가 맡으면 어느 쪽이 뜰지 알 수 없다.
    #[test]
    fn no_extension_is_claimed_twice() {
        let mut seen = std::collections::HashSet::new();
        for s in SERVERS {
            for e in s.exts {
                assert!(seen.insert(*e), "{e}를 두 서버가 맡는다");
            }
        }
    }

    /// 표식을 만나면 **거기서 멈춘다**(가장 가까운 조상).
    #[test]
    fn the_nearest_marker_wins() {
        let base = std::env::temp_dir().join("nabi_lsp_root");
        let deep = base.join("crates").join("app").join("src");
        let _ = std::fs::create_dir_all(&deep);
        std::fs::write(base.join("Cargo.toml"), "").unwrap();
        std::fs::write(base.join("crates").join("Cargo.toml"), "").unwrap();
        let got = project_root(&deep.join("main.rs"), &["Cargo.toml"]).unwrap();
        assert_eq!(got, base.join("crates"), "더 먼 조상을 골랐다");
        let _ = std::fs::remove_dir_all(&base);
    }

    /// 표식이 하나도 없으면 **파일이 있는 폴더**로 — 안 붙는 것보다 낫다.
    #[test]
    fn no_marker_falls_back_to_the_files_own_folder() {
        let dir = std::env::temp_dir().join("nabi_lsp_noroot");
        let _ = std::fs::create_dir_all(&dir);
        let got = project_root(&dir.join("a.py"), &["pyproject.toml"]).unwrap();
        assert_eq!(got, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 부모가 없는 경로에서는 답이 없다(터지지 않는다).
    #[test]
    fn a_bare_name_has_no_root() {
        assert!(project_root(Path::new("a.rs"), &["Cargo.toml"]).is_none());
    }
}
