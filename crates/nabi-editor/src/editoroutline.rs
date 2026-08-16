//! nabiPad 문서 개요(아웃라인) 패널 — 마크다운 헤더/코드 정의 줄을 파싱해
//! 좌측에 목록으로 보여주고, 클릭하면 그 줄로 점프(코드 폴딩 A6의 내비게이션 절반).
//! TextEdit 렌더러를 건드리지 않아 정렬 회귀 위험이 없다(find.scroll_to 재사용).

/// 개요 항목 한 줄.
#[derive(Clone)]
pub struct OutlineItem {
    pub line: usize, // 0-기반 줄 번호(점프 대상).
    pub label: String,
    pub depth: u8, // 들여쓰기 수준(0=최상위).
}

/// 마크다운 헤더 수준(`#`~`######` + 공백). 헤더 아니면 None.
fn md_header_level(t: &str) -> Option<u8> {
    let hashes = t.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes).then_some(()).filter(|_| t.chars().nth(hashes) == Some(' ')).map(|_| hashes as u8)
}

/// 코드 정의 줄이면 표시 라벨(이름/시그니처 앞부분). 아니면 None.
/// 접근/수정자 접두(public/static/…)를 모두 벗기고 흔한 정의 키워드를 검사한다.
/// 다국어 지원: Rust(fn/struct/…)·Go/Swift(func)·Kotlin(fun)·JS/TS(function/class/interface/type/namespace)
/// ·Python(def/class)·Ruby(module/def)·C#/Java(class/record/namespace + 접근 수정자).
fn code_def_label(t: &str) -> Option<String> {
    const KW: [&str; 17] = [
        "fn ", "func ", "fun ", "function ", "def ", "class ", "struct ", "enum ", "trait ", "impl ", "mod ",
        "module ", "interface ", "namespace ", "record ", "type ", "macro_rules!", // Rust 매크로 정의.
    ];
    // 줄 앞 수정자(스택 가능: "public static final class")를 안정될 때까지 반복 제거.
    const PRE: [&str; 12] = [
        "pub ", "async ", "export ", "default ", "static ", "public ", "private ", "protected ", "internal ",
        "open ", "final ", "override ",
    ];
    let mut s = t;
    loop {
        let start = s;
        for pre in PRE {
            s = s.strip_prefix(pre).unwrap_or(s);
        }
        if s == start {
            break; // 더 벗길 접두 없음.
        }
    }
    KW.iter().any(|kw| s.starts_with(kw)).then(|| {
        // 라벨 = 여는 중괄호/괄호/콜론/등호 앞까지(타입 별칭 `type X =` 포함, 과한 길이 컷).
        s.split(['{', '(', ':', '=']).next().unwrap_or(s).trim().chars().take(60).collect()
    })
}

/// 텍스트에서 개요 항목을 뽑는다. 코드 정의가 하나라도 있으면 그것을(코드 파일),
/// 없고 마크다운 헤더가 있으면 헤더를(문서 파일) 쓴다 — `#` 주석 오검출을 줄인다.
pub fn outline_items(text: &str) -> Vec<OutlineItem> {
    let (mut md, mut code) = (Vec::new(), Vec::new());
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if let Some(level) = md_header_level(trimmed) {
            let label = trimmed[level as usize..].trim().to_string();
            if !label.is_empty() {
                md.push(OutlineItem { line: i, label, depth: level - 1 });
            }
        } else if let Some(label) = code_def_label(trimmed) {
            let depth = ((raw.len() - trimmed.len()) / 2).min(6) as u8;
            code.push(OutlineItem { line: i, label, depth });
        }
    }
    if !code.is_empty() {
        code
    } else {
        md
    }
}

/// 개요 패널 — 항목을 수준별로 들여 나열, 클릭하면 그 줄(0기반)을 돌려준다.
pub fn outline_panel(ui: &mut egui::Ui, items: &[OutlineItem]) -> Option<usize> {
    let mut jump = None;
    if items.is_empty() {
        ui.weak("\u{2014}"); // 구조 없음(em dash).
        return None;
    }
    // 심볼이 많을 때(≥10) 필터 박스 — 큰 파일에서 빠른 탐색. 필터 id는 패널 ui 기반이라 탭마다 독립.
    let ql = if items.len() >= 10 {
        let fid = ui.id().with("ofilt");
        let mut q = ui.data(|d| d.get_temp::<String>(fid)).unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut q).hint_text("\u{1f50d}").desired_width(f32::INFINITY));
        ui.data_mut(|d| d.insert_temp(fid, q.clone()));
        q.to_lowercase()
    } else {
        String::new()
    };
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for it in items.iter().filter(|it| ql.is_empty() || it.label.to_lowercase().contains(&ql)) {
            ui.horizontal(|ui| {
                ui.add_space(it.depth as f32 * 10.0);
                if ui.link(&it.label).clicked() {
                    jump = Some(it.line);
                }
            });
        }
    });
    jump
}

#[cfg(test)]
mod tests {
    use super::outline_items;

    #[test]
    fn markdown_headers() {
        let it = outline_items("# Title\nbody\n## Sub\n### Deep\n");
        assert_eq!(it.len(), 3);
        assert_eq!(it[0].label, "Title");
        assert_eq!(it[0].depth, 0);
        assert_eq!(it[2].depth, 2);
        assert_eq!(it[2].line, 3);
    }

    #[test]
    fn code_defs_win_over_hash_comments() {
        // 파이썬: `#` 주석이 있어도 def/class를 개요로(헤더 오검출 회피).
        let it = outline_items("# comment\nclass Foo:\n    def bar(self):\n        pass\n");
        assert_eq!(it.len(), 2);
        assert_eq!(it[0].label, "class Foo");
        assert_eq!(it[1].label, "def bar");
        assert_eq!(it[1].depth, 2); // 4칸 들여쓰기/2.
    }

    #[test]
    fn rust_pub_fn() {
        let it = outline_items("pub fn main() {\n}\nstruct S { a: u8 }\n");
        assert_eq!(it[0].label, "fn main");
        assert_eq!(it[1].label, "struct S");
    }

    #[test]
    fn multilang_defs() {
        // Go: func — 이전에는 전혀 인식 못 했다(주요 언어 결함 수정).
        let go = outline_items("package main\nfunc main() {\n}\nfunc Add(a, b int) int {\n}\n");
        assert_eq!(go[0].label, "func main");
        assert_eq!(go[1].label, "func Add");
        // Kotlin fun, Ruby module, C#/Java 접근 수정자, TS namespace/record.
        assert_eq!(outline_items("fun greet(name: String) {\n}\n")[0].label, "fun greet");
        assert_eq!(outline_items("module MyMod\nend\n")[0].label, "module MyMod");
        assert_eq!(outline_items("public class Foo {\n}\n")[0].label, "class Foo");
        assert_eq!(outline_items("public static final class Bar {}\n")[0].label, "class Bar");
        assert_eq!(outline_items("namespace App.Core {\n}\n")[0].label, "namespace App.Core");
        assert_eq!(outline_items("public record Point(int X, int Y);\n")[0].label, "record Point");
        // Rust 매크로 정의.
        assert_eq!(outline_items("macro_rules! my_vec {\n    () => {};\n}\n")[0].label, "macro_rules! my_vec");
    }

    #[test]
    fn type_alias_label() {
        assert_eq!(outline_items("type Id = u64;\n")[0].label, "type Id"); // 등호 앞까지.
    }

    #[test]
    fn empty_when_plain() {
        assert!(outline_items("just some text\nno structure here\n").is_empty());
    }
}
