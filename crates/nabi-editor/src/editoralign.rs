//! nabiPad 정렬 도구(Sublime "Alignment"/EmEditor·`column -t` 벤치마킹) — 줄들을 보기 좋게 맞춤.
//! `=` 기준 정렬과 공백 구분 열 정렬. 순수 함수라 단위 테스트로 검증한다.

/// 첫 `=` 위치를 맞춰 대입문을 정렬한다(좌변을 공통 폭으로 패딩). `=` 없는 줄은 유지.
pub fn align_equals(t: &str) -> String {
    let lines: Vec<&str> = t.split('\n').collect();
    let width = lines
        .iter()
        .filter_map(|l| l.find('=').map(|i| l[..i].trim_end().chars().count()))
        .max()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| match l.find('=') {
            Some(i) => {
                let lhs = l[..i].trim_end();
                let rhs = l[i + 1..].trim_start();
                let pad = width.saturating_sub(lhs.chars().count());
                format!("{lhs}{} = {rhs}", " ".repeat(pad))
            }
            None => l.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 공백으로 나뉜 열을 표처럼 정렬한다(각 열을 최대 폭으로 패딩, 두 칸 간격). 빈 줄은 유지.
pub fn align_columns(t: &str) -> String {
    let rows: Vec<Vec<&str>> = t.split('\n').map(|l| l.split_whitespace().collect()).collect();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, cell)| {
                    if i + 1 == row.len() {
                        (*cell).to_string() // 마지막 열은 패딩하지 않음(후행 공백 방지).
                    } else {
                        format!("{cell}{}", " ".repeat(widths[i] - cell.chars().count()))
                    }
                })
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_alignment() {
        assert_eq!(align_equals("a=1\nbbb=2\ncc = 3"), "a   = 1\nbbb = 2\ncc  = 3");
        assert_eq!(align_equals("no equals\nx=1"), "no equals\nx = 1"); // `=` 없는 줄 유지.
    }

    #[test]
    fn column_alignment() {
        assert_eq!(align_columns("a bb c\nxxx y z"), "a    bb  c\nxxx  y   z");
        assert_eq!(align_columns("one two"), "one  two");
    }
}
