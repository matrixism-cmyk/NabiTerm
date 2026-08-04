//! 외부 편집기 편집 세션.

use nabi_fs::RemoteFs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 한 원격 파일의 편집 세션. drop 시 임시 파일을 정리한다.
pub struct EditSession {
    fs: Box<dyn RemoteFs>,
    remote_path: String,
    temp_path: PathBuf,
    last: Vec<u8>,
}

impl EditSession {
    /// 원격 파일을 임시 디렉터리로 받아 편집 세션을 시작한다.
    pub async fn start(
        mut fs: Box<dyn RemoteFs>,
        remote_path: &str,
        temp_dir: &Path,
    ) -> Result<EditSession, String> {
        let content = fs.read_file(remote_path).await?;
        std::fs::create_dir_all(temp_dir).map_err(|e| e.to_string())?;
        let name = Path::new(remote_path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "edit.tmp".to_owned());
        let temp_path = temp_dir.join(name);
        std::fs::write(&temp_path, &content).map_err(|e| e.to_string())?;
        Ok(EditSession {
            fs,
            remote_path: remote_path.to_owned(),
            temp_path,
            last: content,
        })
    }

    /// 설정된 편집기로 임시 파일을 연다(VS Code 등 탭형은 `code --wait` 권장).
    pub fn launch_editor(&self, editor: &str) -> Result<(), String> {
        Command::new(editor)
            .arg(&self.temp_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// 임시 파일이 바뀌었으면 원격으로 재업로드한다. 변경되었으면 true.
    pub async fn reupload_if_changed(&mut self) -> Result<bool, String> {
        let current = std::fs::read(&self.temp_path).map_err(|e| e.to_string())?;
        if current == self.last {
            return Ok(false);
        }
        self.fs.write_file(&self.remote_path, &current).await?;
        self.last = current;
        Ok(true)
    }

    pub fn temp_path(&self) -> &Path {
        &self.temp_path
    }
}

impl Drop for EditSession {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temp_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabi_fs::LocalFs;

    #[tokio::test]
    async fn edit_cycle_reuploads_only_on_change() {
        let base = std::env::temp_dir().join(format!("nabi-editor-{}", std::process::id()));
        let remote = base.join("remote.txt");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(&remote, b"old").unwrap();

        let tmp_dir = base.join("tmp");
        let mut sess = EditSession::start(Box::new(LocalFs), remote.to_str().unwrap(), &tmp_dir)
            .await
            .unwrap();

        // 편집기가 임시 파일을 수정했다고 가정.
        std::fs::write(sess.temp_path(), b"new content").unwrap();
        assert!(sess.reupload_if_changed().await.unwrap());
        assert_eq!(std::fs::read(&remote).unwrap(), b"new content");

        // 변경 없음 → 재업로드 안 함.
        assert!(!sess.reupload_if_changed().await.unwrap());

        drop(sess);
        let _ = std::fs::remove_dir_all(&base);
    }
}
