//! 테스트용 인-프로세스 SSH+SFTP 서버 하네스(러 ssh-sftp 예제 패턴 차용).
//!
//! 외부 서버 없이 SftpFs(list/read/write/remove/rename/mkdir) 왕복을 검증한다.
//! 테스트 본문은 sftp_test.rs.

use crate::connect_sftp;
use nabi_proto::SshParams;
use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==
-----END OPENSSH PRIVATE KEY-----
";

#[derive(Default)]
struct SshSession {
    clients: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
}

impl server::Handler for SshSession {
    type Error = russh::Error;

    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        _s: &mut Session,
    ) -> Result<bool, Self::Error> {
        self.clients.lock().await.insert(channel.id(), channel);
        Ok(true)
    }

    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name == "sftp" {
            let channel = self.clients.lock().await.remove(&id).unwrap();
            session.channel_success(id)?;
            russh_sftp::server::run(channel.into_stream(), Sftp::new()).await;
        } else {
            session.channel_failure(id)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Sftp {
    listed: bool,
    files: HashMap<String, Vec<u8>>,
    /// setstat로 설정된 권한(chmod 라운드트립 검증용).
    perms: HashMap<String, u32>,
}

impl Sftp {
    /// readdir가 돌려주는 foo.txt/bar를 읽을 수 있게 내용을 미리 채운다.
    fn new() -> Self {
        let mut files = HashMap::new();
        files.insert("/foo.txt".to_string(), b"foo".to_vec());
        files.insert("/bar".to_string(), b"bar".to_vec());
        Self {
            listed: false,
            files,
            perms: HashMap::new(),
        }
    }
}

fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

impl russh_sftp::server::Handler for Sftp {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn close(&mut self, id: u32, _handle: String) -> Result<Status, Self::Error> {
        Ok(ok_status(id))
    }

    async fn realpath(&mut self, id: u32, _path: String) -> Result<Name, Self::Error> {
        Ok(Name { id, files: vec![File::dummy("/")] })
    }

    async fn mkdir(
        &mut self,
        id: u32,
        _path: String,
        _attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(ok_status(id))
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        if let Some(p) = attrs.permissions {
            self.perms.insert(path, p);
        }
        Ok(ok_status(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        self.listed = false;
        Ok(Handle { id, handle: path })
    }

    async fn readdir(&mut self, id: u32, _handle: String) -> Result<Name, Self::Error> {
        if self.listed {
            return Err(StatusCode::Eof);
        }
        self.listed = true;
        // 정규 파일(S_IFREG) + setstat로 바뀐 권한 반영(없으면 기본 0o100644).
        let files = [("foo.txt", "/foo.txt"), ("bar", "/bar")]
            .iter()
            .map(|(name, full)| {
                let p = self.perms.get(*full).copied().unwrap_or(0o100_644);
                let sz = self.files.get(*full).map(|v| v.len() as u64).unwrap_or(0);
                File::new(
                    *name,
                    FileAttributes {
                        size: Some(sz), // 실제 크기 반환(dir_size 등 검증용).
                        permissions: Some(p),
                        mtime: Some(1_700_000_000), // 고정 mtime(라운드트립 검증용).
                        ..Default::default()
                    },
                )
            })
            .collect();
        Ok(Name { id, files })
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        pflags: OpenFlags,
        _attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        if pflags.contains(OpenFlags::WRITE) {
            self.files.insert(filename.clone(), Vec::new());
        } else if !self.files.contains_key(&filename) {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Handle { id, handle: filename })
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let content = self.files.get(&handle).cloned().unwrap_or_default();
        let off = offset as usize;
        if off >= content.len() {
            return Err(StatusCode::Eof);
        }
        let end = (off + len as usize).min(content.len());
        Ok(Data {
            id,
            data: content[off..end].to_vec(),
        })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let buf = self.files.entry(handle).or_default();
        let off = offset as usize;
        if buf.len() < off + data.len() {
            buf.resize(off + data.len(), 0);
        }
        buf[off..off + data.len()].copy_from_slice(&data);
        Ok(ok_status(id))
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        self.files.remove(&filename);
        Ok(ok_status(id))
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        if let Some(content) = self.files.remove(&oldpath) {
            self.files.insert(newpath, content);
        }
        Ok(ok_status(id))
    }
}

/// 인프로세스 SSH+SFTP 서버를 띄우고 접속 주소를 돌려준다.
async fn start_server() -> std::net::SocketAddr {
    let key = russh::keys::PrivateKey::from_openssh(SERVER_KEY).unwrap();
    let config = Arc::new(server::Config {
        keys: vec![key],
        ..Default::default()
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((stream, _)) = listener.accept().await {
            if let Ok(rs) = server::run_stream(config, stream, SshSession::default()).await {
                let _ = rs.await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(250)).await;
    addr
}

/// 인프로세스 서버에 접속해 SftpFs를 돌려준다(테스트 진입점).
pub(crate) async fn connect_fs() -> crate::SftpFs {
    let addr = start_server().await;
    let params = SshParams::password(addr.ip().to_string(), addr.port(), "u", "p");
    connect_sftp(&params).await.expect("sftp connect")
}
