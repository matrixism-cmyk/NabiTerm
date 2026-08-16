//! nabiPad CSV 변환 보조(editorcsv 라인 한도 분리) — 마크다운 표 → CSV.

use crate::editorcsv::{parse_csv, write_csv};

/// CSV → TSV(필드 내 탭/개행은 공백 치환).
pub fn csv_to_tsv(t: &str) -> String {
    parse_csv(t, ',')
        .iter()
        .map(|r| r.iter().map(|f| f.replace(['\t', '\n'], " ")).collect::<Vec<_>>().join("\t"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// TSV → CSV.
pub fn tsv_to_csv(t: &str) -> String {
    let rows: Vec<Vec<String>> = t.split('\n').map(|l| l.split('\t').map(str::to_string).collect()).collect();
    write_csv(&rows, ',')
}

/// 마크다운 표 → CSV. 파이프(|) 구분, 구분선 행(`--- | :--:`)은 건너뛴다. write_csv로 안전 인용.
pub fn md_to_csv(t: &str) -> String {
    let rows: Vec<Vec<String>> = t
        .lines()
        .map(str::trim)
        .filter(|l| l.contains('|'))
        .map(|l| l.trim_matches('|').split('|').map(|c| c.trim().replace("\\|", "|")).collect::<Vec<_>>())
        .filter(|cells: &Vec<String>| {
            // 헤더 아래 구분선 행(--- | :--:)은 제외.
            !cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
        })
        .collect();
    if rows.is_empty() {
        return t.to_string();
    }
    write_csv(&rows, ',')
}

#[cfg(test)]
mod tests {
    use super::md_to_csv;

    #[test]
    fn md_table_to_csv() {
        assert_eq!(md_to_csv("| h1 | h2 |\n| --- | --- |\n| v1 | v2 |"), "h1,h2\nv1,v2"); // 구분선 제외.
        assert_eq!(md_to_csv("| a, b | c |\n|---|---|\n| x | y |"), "\"a, b\",c\nx,y"); // 쉼표 셀 인용.
        assert_eq!(md_to_csv("no table here"), "no table here"); // 표 없으면 원문.
    }
}
