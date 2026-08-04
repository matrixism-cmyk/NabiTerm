//! 파일 확장자 → 표시 아이콘·색(로컬·SFTP 브라우저 공용). 폴더는 호출측이 📁 처리.
//! 확장자→카테고리 매핑은 [`category`] 한 곳에서만(아이콘·색 드리프트 방지, SSOT).

/// 파일 유형 카테고리(아이콘·색의 공통 분류).
enum Cat {
    Image,
    Archive,
    Code,
    Doc,
    Office,
    Media,
    Pdf,
    Exec,
    Other,
}

/// 파일 이름의 확장자를 카테고리로 분류한다(대소문자 무시). 아이콘·색이 공유.
fn category(name: &str) -> Cat {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico" => Cat::Image,
        "zip" | "tar" | "gz" | "tgz" | "rar" | "7z" | "xz" | "bz2" | "zst" => Cat::Archive,
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "go" | "c" | "cpp" | "cc" | "h" | "hpp"
        | "java" | "kt" | "swift" | "cs" | "rb" | "php" | "lua" | "sql" | "scala" | "dart"
        | "sh" | "bash" | "zsh" | "pl" | "r" | "vue" | "toml" | "json" | "yaml" | "yml"
        | "html" | "css" | "scss" | "less" | "xml" => Cat::Code,
        "md" | "markdown" | "rst" | "adoc" => Cat::Doc,
        "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => Cat::Office,
        "mp3" | "wav" | "flac" | "ogg" | "mp4" | "mkv" | "avi" | "mov" => Cat::Media,
        "pdf" => Cat::Pdf,
        "exe" | "msi" | "dll" | "bin" | "so" => Cat::Exec,
        _ => Cat::Other,
    }
}

/// 파일 이름의 확장자에 맞는 아이콘(없으면 기본 문서 아이콘).
pub(crate) fn file_icon(name: &str) -> &'static str {
    match category(name) {
        Cat::Image => "\u{1f5bc}",  // 🖼
        Cat::Archive => "\u{1f4e6}", // 📦
        Cat::Code => "\u{1f4dc}",   // 📜 코드/마크업
        Cat::Doc => "\u{1f4dd}",    // 📝 문서/마크다운
        Cat::Office => "\u{1f4d8}", // 📘 오피스
        Cat::Media => "\u{1f3ac}",  // 🎬
        Cat::Pdf => "\u{1f4d5}",    // 📕
        Cat::Exec => "\u{2699}",    // ⚙
        Cat::Other => "\u{1f4c4}",  // 📄
    }
}

/// 폴더 이름/아이콘 색(따뜻한 금색 — 탐색기식).
pub(crate) const FOLDER_COLOR: egui::Color32 = egui::Color32::from_rgb(0xe6, 0xc3, 0x5c);

/// 파일 유형 카테고리별 색(컬러풀한 목록 — lsd/exa식). 아이콘과 동일 분류(SSOT).
pub(crate) fn file_color(name: &str) -> egui::Color32 {
    use egui::Color32;
    match category(name) {
        Cat::Image => Color32::from_rgb(0xc5, 0x8a, 0xf0),          // 보라(이미지)
        Cat::Archive => Color32::from_rgb(0xe0, 0x9b, 0x4a),        // 주황(압축)
        Cat::Code => Color32::from_rgb(0x6a, 0xc9, 0x7a),           // 초록(코드)
        Cat::Doc | Cat::Office => Color32::from_rgb(0x6a, 0x9f, 0xd0), // 파랑(문서)
        Cat::Media => Color32::from_rgb(0xe8, 0x7d, 0x9a),          // 분홍(미디어)
        Cat::Pdf => Color32::from_rgb(0xe0, 0x6c, 0x6c),            // 빨강
        Cat::Exec => Color32::from_rgb(0x7a, 0xc7, 0xd0),           // 청록(실행)
        Cat::Other => Color32::from_rgb(0xcd, 0xcd, 0xcd),          // 기본 회색
    }
}

#[cfg(test)]
mod tests {
    use super::file_icon;

    #[test]
    fn icon_by_ext() {
        assert_eq!(file_icon("a.png"), "\u{1f5bc}");
        assert_eq!(file_icon("src/main.rs"), "\u{1f4dc}");
        assert_eq!(file_icon("pack.tar.gz"), "\u{1f4e6}");
        assert_eq!(file_icon("doc.pdf"), "\u{1f4d5}");
        assert_eq!(file_icon("README"), "\u{1f4c4}"); // 확장자 없음 → 기본.
        assert_eq!(file_icon("movie.MP4"), "\u{1f3ac}"); // 대소문자 무시.
        assert_eq!(file_icon("App.tsx"), "\u{1f4dc}"); // 확장 코드(tsx).
        assert_eq!(file_icon("README.md"), "\u{1f4dd}"); // 마크다운.
        assert_eq!(file_icon("report.docx"), "\u{1f4d8}"); // 오피스 문서.
    }

    #[test]
    fn color_matches_icon_category() {
        use super::file_color;
        // 같은 카테고리는 아이콘·색이 함께 분류된다(드리프트 방지).
        assert_eq!(file_color("a.kt"), file_color("a.rs")); // 둘 다 코드색.
        assert_ne!(file_color("a.kt"), file_color("a.png")); // 코드≠이미지.
        assert_eq!(file_color("a.md"), file_color("a.docx")); // 문서색 공유.
    }
}
