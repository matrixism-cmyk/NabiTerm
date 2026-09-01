//! **삼킨 오류**를 찾는다 — 저장·쓰기가 실패해도 아무도 모르는 자리.
//!
//! ## 왜 필요한가
//!
//! `let _ = nabi_config::save(...)` 는 컴파일도 되고 시험도 통과한다. 디스크가 차거나
//! 설정 폴더가 읽기 전용이면 **조용히 실패**하고, 사용자는 다시 켰을 때 바꿔 둔 것이
//! 전부 돌아가 있는 것을 본다. 그때는 무엇이 언제 실패했는지 알 방법이 없다.
//!
//! 2026-08-28에 이런 자리 마흔두 곳을 한 곳(`savecfg.rs`)으로 모았다. 그런데
//! **모으는 것만으로는 지켜지지 않았다** — 두 달 만에 여덟 곳이 다시 생겼고, nabiPad
//! 설정은 네 곳 전부가 삼키고 있었다. 모았는지 지켜보는 검사가 있어야 한다.
//!
//! ## 무엇을 세는가
//!
//! `let _ =` 로 버리는 값 가운데 **저장·쓰기로 보이는 것**만 센다. 채널 보내기나
//! 창 닫기 같은 것은 실패해도 사용자가 잃는 것이 없어 여기 넣지 않는다 — 넓게 잡으면
//! 경고가 수백 건이 되고, 수백 건은 아무도 안 본다.
//!
//! ## 빠져나가는 길
//!
//! 정말로 삼켜야 하는 자리가 있다. 끄는 중이거나 창이 뜨기 전이면 **알릴 화면이 없다.**
//! 그때는 바로 윗줄에 `// 삼킴:` 으로 이유를 적는다. 예외 목록을 따로 두지 않는 이유는
//! 목록이 코드에서 떨어져 있으면 곧 낡기 때문이다 — 이유는 코드 옆에 있어야 한다.

use std::path::Path;

/// 저장·쓰기로 보는 이름들.
///
/// **파일로 남기는 것만** 본다. 처음에는 `write_all`·`flush`·`create_dir_all` 까지
/// 넣었더니 쉰아홉 건이 나왔는데, 대부분이 네트워크로 보내는 자리(LSP·FTP)이거나
/// 곧바로 뒤따르는 쓰기가 어차피 실패를 알리는 폴더 만들기였다. 섞여 있으면 진짜
/// 열아홉 건이 묻힌다.
/// **지우는 것도 넣는다**(2026-09-01). 로컬 탐색기의 삭제가 `let _ = trash::delete(...)`
/// 로 실패를 삼키고 있었는데 이 목록에 없어서 검사기가 못 봤다. 휴지통이 없는 자리
/// (네트워크 드라이브·일부 이동식)에서는 **아무 일도 안 일어나는데 사람은 지운 줄 안다.**
/// 원격 SFTP 쪽은 실패를 알리고 있어서, 같은 동작이 창에 따라 다르게 굴었다.
const SAVERS: &[&str] = &[
    "fs::write",
    "fs::rename",
    "fs::copy",
    "trash::delete",
    "save_tree(",
    "nabi_config::save(",
];

/// 이 파일은 보지 않는다 — 시험과 예제.
///
/// 시험은 임시 파일을 만들며 버리는 자리가 많다. 게다가 거기서 저장이 실패하면 그
/// 시험이 통과하지 못하므로 곧바로 드러난다 — 조용히 지나가지 않는다.
/// 예제는 사람이 손으로 돌리는 진단 도구라 실패가 화면에 그대로 보인다.
fn skip_file(path: &str) -> bool {
    let p = path.replace('\\', "/");
    p.contains("/tests/") || p.contains("/examples/") || p.ends_with("_test.rs")
}

pub fn run() -> std::process::ExitCode {
    let files = crate::rswalk::rust_files(Path::new("crates"));
    let mut hits: Vec<(String, usize, String)> = Vec::new();
    for (path, text) in &files {
        if skip_file(path) {
            continue;
        }
        scan(path, text, &mut hits);
    }
    println!("검사 {} 파일 · 삼킨 오류 {}", files.len(), hits.len());
    for (f, n, line) in &hits {
        println!("warn: {f}:{n} = {}", line.trim());
    }
    if !hits.is_empty() {
        println!("→ 알릴 수 있으면 알리고, 알릴 화면이 없으면 윗줄에 `// 삼킴: 이유` 를 적을 것");
    }
    std::process::ExitCode::SUCCESS
}

fn scan(path: &str, text: &str, hits: &mut Vec<(String, usize, String)>) {
    // 시험 코드는 세지 않는다 — 거기서 저장이 실패하면 그 시험이 통과하지 못하므로
    // 조용히 지나가지 않는다.
    let lines: Vec<&str> = crate::rswalk::without_tests(text).lines().collect();
    for (i, line) in lines.iter().enumerate() {
        // 주석은 코드가 아니다 — 이 결함을 설명해 둔 글이 스스로 걸리곤 했다.
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        if !line.contains("let _ = ") || !SAVERS.iter().any(|s| line.contains(s)) {
            continue;
        }
        if excused(&lines, i) {
            continue;
        }
        hits.push((path.to_string(), i + 1, (*line).to_string()));
    }
}

/// 바로 위 주석 줄에 `// 삼킴:` 이 있으면 이유를 적은 것으로 본다.
fn excused(lines: &[&str], i: usize) -> bool {
    (1..=2).any(|back| {
        i.checked_sub(back)
            .and_then(|k| lines.get(k))
            .is_some_and(|l| l.contains("삼킴:"))
    })
}

#[cfg(test)]
mod tests {
    use super::{excused, skip_file};

    #[test]
    fn 시험과_예제는_보지_않는다() {
        assert!(skip_file("crates/nabi-trzsz/tests/real.rs"));
        assert!(skip_file(r"crates\nabi-pty\examples\seqprobe.rs"));
        assert!(skip_file("crates/nabi-ftp/src/ftp_test.rs"));
        assert!(!skip_file("crates/nabi-app/src/worklayout.rs"));
    }

    #[test]
    fn 바로_위에_이유가_있으면_넘어간다() {
        let l = ["// 삼킴: 알릴 화면이 없다", "    let _ = fs::write(p, s);"];
        assert!(excused(&l, 1));
    }

    #[test]
    fn 이유가_없으면_센다() {
        let l = ["// 설정을 적는다", "    let _ = fs::write(p, s);"];
        assert!(!excused(&l, 1));
    }

    #[test]
    fn 첫줄이면_위를_보지_않는다() {
        // 일부러 깨 본 자리다 — 인덱스가 음수로 내려가면 여기서 터진다.
        assert!(!excused(&["let _ = fs::write(p, s);"], 0));
    }
}
