//! git worktree 통합(B6) — 병렬 에이전트 작업의 표준 패턴(herdr·OpenClaw 공통).
//!
//! 브랜치별 워크트리를 만들어 새 탭(cwd=워크트리)으로 연다. 에이전트 여럿이 같은 저장소를
//! 만질 때 작업 트리 오염 없이 각자 브랜치에서 진행한다. git CLI를 호출한다(런타임 의존 없음).

use std::path::{Path, PathBuf};

/// `git worktree list --porcelain` 한 항목.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Wt {
    pub path: String,
    pub branch: String,
}

/// porcelain 출력 파싱(순수). bare 저장소 항목은 뺀다.
pub(crate) fn parse_worktree_list(out: &str) -> Vec<Wt> {
    let mut items = Vec::new();
    let (mut path, mut branch, mut bare) = (None::<String>, String::new(), false);
    for line in out.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if let Some(p) = path.take() {
                if !bare {
                    items.push(Wt { path: p, branch: std::mem::take(&mut branch) });
                }
            }
            (branch, bare) = (String::new(), false);
        } else if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(p.to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = b.trim_start_matches("refs/heads/").to_string();
        } else if line == "bare" {
            bare = true;
        } else if line == "detached" {
            branch = "(detached)".into();
        }
    }
    items
}

/// 새 워크트리 경로: 저장소 **형제 폴더** `<repo이름>-wt/<브랜치>` — 저장소 안을 어지럽히지
/// 않으면서 탐색기에서 바로 보인다. 브랜치의 `/`는 폴더 구분자가 되지 않게 `-`로 바꾼다.
pub(crate) fn worktree_path(repo_root: &Path, branch: &str) -> PathBuf {
    let name = repo_root.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into());
    let flat = branch.replace(['/', '\\'], "-");
    repo_root.parent().unwrap_or(repo_root).join(format!("{name}-wt")).join(flat)
}

/// git을 창 없이 실행해 (성공, stdout+stderr)를 돌려준다.
pub(crate) fn run_git(cwd: &str, args: &[&str]) -> (bool, String) {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("git")
        .arg("-C").arg(cwd).args(args)
        .creation_flags(0x0800_0000)
        .output();
    match out {
        Ok(o) => {
            let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            (o.status.success(), text)
        }
        Err(e) => (false, e.to_string()),
    }
}

/// 저장소 루트(`git rev-parse --show-toplevel`). 저장소가 아니면 None.
pub(crate) fn repo_root(cwd: &str) -> Option<PathBuf> {
    let (ok, out) = run_git(cwd, &["rev-parse", "--show-toplevel"]);
    ok.then(|| PathBuf::from(out.trim()))
}

/// 워크트리 생성: 새 브랜치 `-b`(이미 있으면 기존 브랜치 체크아웃 폴백). 성공 시 경로 반환.
pub(crate) fn create(cwd: &str, branch: &str) -> Result<PathBuf, String> {
    let root = repo_root(cwd).ok_or_else(|| "git 저장소가 아닙니다".to_string())?;
    let dest = worktree_path(&root, branch);
    let dest_s = dest.to_string_lossy().into_owned();
    let (ok, out) = run_git(cwd, &["worktree", "add", &dest_s, "-b", branch]);
    if ok {
        return Ok(dest);
    }
    // 브랜치가 이미 있으면 -b가 실패한다 — 기존 브랜치로 체크아웃 재시도.
    let (ok2, out2) = run_git(cwd, &["worktree", "add", &dest_s, branch]);
    if ok2 { Ok(dest) } else { Err(format!("{out}{out2}")) }
}

pub(crate) fn list(cwd: &str) -> Result<Vec<Wt>, String> {
    let (ok, out) = run_git(cwd, &["worktree", "list", "--porcelain"]);
    if ok { Ok(parse_worktree_list(&out)) } else { Err(out) }
}

pub(crate) fn remove(cwd: &str, path: &str) -> Result<(), String> {
    let (ok, out) = run_git(cwd, &["worktree", "remove", path]);
    if ok { Ok(()) } else { Err(out) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_output() {
        let out = "worktree C:/proj\nHEAD abc\nbranch refs/heads/main\n\n\
                   worktree C:/proj-wt/feat-x\nHEAD def\nbranch refs/heads/feat/x\n\n\
                   worktree C:/bare.git\nbare\n\n\
                   worktree C:/det\nHEAD 123\ndetached\n";
        let items = parse_worktree_list(out);
        assert_eq!(items.len(), 3, "bare는 제외");
        assert_eq!(items[0], Wt { path: "C:/proj".into(), branch: "main".into() });
        assert_eq!(items[1].branch, "feat/x");
        assert_eq!(items[2].branch, "(detached)");
    }

    /// 워크트리는 저장소 밖(형제 폴더)에 만든다 — 저장소 안이면 자기 자신을 오염시킨다.
    #[test]
    fn path_is_sibling_and_flattens_branch() {
        let p = worktree_path(Path::new(r"C:\work\nabi"), "feat/scroll");
        assert_eq!(p, Path::new(r"C:\work\nabi-wt\feat-scroll"));
        assert!(!p.starts_with(r"C:\work\nabi"), "저장소 내부 금지");
    }
}
