//! **서버 안에서 폴더째 복사**(재귀). 파일 하나 복사는 `fs.rs`의 `copy_remote`가 한다.
//!
//! 파일만 되던 것을 폴더로 넓힌다. 지금까지 폴더는 **받았다가 다시 올려야** 했다 —
//! 같은 기계 안에서 도는 일에 회선을 두 번 태우는 셈이었다.
//!
//! ## 자기 안으로 복사하는 것을 먼저 막는다
//!
//! `/a`를 `/a/b`로 복사하면, 만들어진 것을 다시 읽으면서 끝없이 내려간다. 서버 디스크가
//! 찰 때까지 멈추지 않는다. 그래서 걷기 전에 순수 함수로 걸러 내고, 그 함수에 시험을 붙인다.

use crate::fs::SftpFs;
use crate::recurse::DirProgress;
use nabi_fs::{FileKind, RemoteFs};

/// 대상이 원본 자신이거나 **그 아래**인가 — 그렇다면 복사하면 안 된다.
///
/// 경로를 조각으로 나눠 견준다. 글자로만 견주면 `/ab`가 `/a` 아래로 잘못 읽힌다.
pub fn is_inside(src: &str, dst: &str) -> bool {
    let parts = |p: &str| -> Vec<String> {
        p.split('/').filter(|s| !s.is_empty() && *s != ".").map(str::to_string).collect()
    };
    let (s, d) = (parts(src), parts(dst));
    d.len() >= s.len() && d[..s.len()] == s[..]
}

impl SftpFs {
    /// 원격 폴더를 같은 서버의 다른 자리로 재귀 복사한다.
    ///
    /// 폴더는 먼저 만들고(있으면 그냥 둔다) 안을 하나씩 옮긴다. 링크는 **따라가지 않고
    /// 건너뛴다** — 따라가면 고리에 빠지거나 폴더 밖을 복사하게 된다.
    pub fn copy_dir_remote<'a>(
        &'a mut self,
        from: &'a str,
        to: &'a str,
        prog: &'a mut DirProgress<'_>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(async move {
            if is_inside(from, to) {
                return Err("sftp.copy.intoself".to_string());
            }
            let _ = self.mkdir(to).await; // 있으면 무시(업로드와 같은 규칙).
            for e in self.list_dir(from).await? {
                if e.name == "." || e.name == ".." {
                    continue;
                }
                let src = format!("{}/{}", from.trim_end_matches('/'), e.name);
                let dst = format!("{}/{}", to.trim_end_matches('/'), e.name);
                match e.kind {
                    FileKind::Dir => self.copy_dir_remote(&src, &dst, prog).await?,
                    // 링크는 건너뛴다 — 따라가면 고리에 빠진다. 조용히 넘기지 않고 세어 둔다.
                    FileKind::Symlink | FileKind::LinkDir | FileKind::Other => prog.skipped += 1,
                    FileKind::File => {
                        let size = e.size;
                        self.copy_remote(&src, &dst, &mut |b| prog.report(b)).await?;
                        prog.done += size;
                        prog.report(0);
                    }
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::is_inside;

    /// **이 시험이 이 파일의 존재 이유다** — 자기 안으로 복사하면 끝나지 않는다.
    #[test]
    fn copying_a_folder_into_itself_is_refused() {
        assert!(is_inside("/srv/app", "/srv/app/backup"));
        assert!(is_inside("/srv/app", "/srv/app"));
    }

    /// 이름이 겹치는 옆 폴더는 **안이 아니다**(글자로만 견주면 틀린다).
    #[test]
    fn a_sibling_with_a_shared_prefix_is_not_inside() {
        assert!(!is_inside("/srv/app", "/srv/application"));
        assert!(!is_inside("/a", "/ab"));
    }

    #[test]
    fn a_plain_other_folder_is_allowed() {
        assert!(!is_inside("/srv/app", "/srv/copy"));
        assert!(!is_inside("/srv/app/sub", "/srv/app2"));
    }

    /// 끝의 빗금·겹빗금이 판단을 바꾸면 안 된다.
    #[test]
    fn trailing_and_doubled_slashes_do_not_change_the_answer() {
        assert!(is_inside("/srv/app/", "/srv/app//backup"));
        assert!(!is_inside("/srv/app/", "/srv/other/"));
    }

    /// 위로 올라가는 복사는 막을 일이 아니다(그건 안이 아니다).
    #[test]
    fn copying_up_into_the_parent_is_not_inside() {
        assert!(!is_inside("/srv/app/sub", "/srv/app"));
    }
}
