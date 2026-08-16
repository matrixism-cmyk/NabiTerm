//! FtpFs — RemoteFs를 FTP로 구현(suppaftp, tokio).

use async_trait::async_trait;
use nabi_fs::{FileEntry, FileKind, RemoteFs};
use suppaftp::tokio::AsyncFtpStream;
use tokio::io::AsyncReadExt;

/// FTP 백엔드.
pub struct FtpFs {
    ftp: AsyncFtpStream,
}

/// FTP 서버에 연결·로그인한다(평문 FTP). 런타임은 FTP 서버 필요.
pub async fn connect_ftp(host: &str, port: u16, user: &str, pass: &str) -> Result<FtpFs, String> {
    // 죽은 호스트에서 무한 대기하지 않도록 연결에 타임아웃(15초).
    let connect = AsyncFtpStream::connect(format!("{host}:{port}"));
    let mut ftp = tokio::time::timeout(std::time::Duration::from_secs(15), connect)
        .await
        .map_err(|_| nabi_i18n::trc("net.ftp.timeout").to_string())?
        .map_err(|e| e.to_string())?;
    ftp.login(user, pass).await.map_err(|e| e.to_string())?;
    Ok(FtpFs { ftp })
}

#[async_trait]
impl RemoteFs for FtpFs {
    async fn list_dir(&mut self, path: &str) -> Result<Vec<FileEntry>, String> {
        let lines = self.ftp.list(Some(path)).await.map_err(|e| e.to_string())?;
        Ok(lines.iter().map(|l| parse_ls_line(l)).collect())
    }

    async fn read_file(&mut self, path: &str) -> Result<Vec<u8>, String> {
        let mut stream = self.ftp.retr_as_stream(path).await.map_err(|e| e.to_string())?;
        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .await
            .map_err(|e| e.to_string())?;
        self.ftp
            .finalize_retr_stream(stream)
            .await
            .map_err(|e| e.to_string())?;
        Ok(buf)
    }

    async fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), String> {
        let mut reader: &[u8] = data;
        self.ftp
            .put_file(path, &mut reader)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    async fn remove(&mut self, path: &str) -> Result<(), String> {
        match self.ftp.rm(path).await {
            Ok(()) => Ok(()),
            Err(_) => self.ftp.rmdir(path).await.map_err(|e| e.to_string()),
        }
    }

    async fn rename(&mut self, from: &str, to: &str) -> Result<(), String> {
        self.ftp.rename(from, to).await.map_err(|e| e.to_string())
    }

    async fn mkdir(&mut self, path: &str) -> Result<(), String> {
        self.ftp.mkdir(path).await.map_err(|e| e.to_string())
    }
}

/// `ls -l` 한 줄을 FileEntry로 파싱한다(이름에 공백이 없다고 가정하는 단순 파서).
fn parse_ls_line(line: &str) -> FileEntry {
    let kind = match line.chars().next().unwrap_or('-') {
        'd' => FileKind::Dir,
        'l' => FileKind::Symlink,
        '-' => FileKind::File,
        _ => FileKind::Other,
    };
    let parts: Vec<&str> = line.split_whitespace().collect();
    FileEntry {
        name: parts.last().copied().unwrap_or("").to_owned(),
        kind,
        size: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
        mode: parse_perms(parts.first().copied().unwrap_or("")),
        mtime: 0, // ls 날짜 파싱은 형식이 다양해 생략.
    }
}

/// `ls -l` 권한 필드("-rwxr-xr-x")의 9비트를 POSIX 8진 mode로 변환(파싱 실패=0).
fn parse_perms(field: &str) -> u32 {
    let b = field.as_bytes();
    if b.len() < 10 {
        return 0;
    }
    let bits = [0o400, 0o200, 0o100, 0o040, 0o020, 0o010, 0o004, 0o002, 0o001];
    let mut m = 0u32;
    for (i, &bit) in bits.iter().enumerate() {
        if b[i + 1] != b'-' {
            m |= bit;
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dir_and_file_lines() {
        let d = parse_ls_line("drwxr-xr-x 2 u g 4096 Jan 1 12:00 mydir");
        assert_eq!(d.kind, FileKind::Dir);
        assert_eq!(d.name, "mydir");

        let f = parse_ls_line("-rw-r--r-- 1 u g 1234 Jan 1 12:00 file.txt");
        assert_eq!(f.kind, FileKind::File);
        assert_eq!(f.size, 1234);
        assert_eq!(f.name, "file.txt");
    }

    #[test]
    fn parse_perms_to_octal() {
        assert_eq!(parse_perms("drwxr-xr-x"), 0o755);
        assert_eq!(parse_perms("-rw-r--r--"), 0o644);
        assert_eq!(parse_perms("----------"), 0);
        assert_eq!(parse_perms("short"), 0); // 10자 미만 → 0.
    }
}
