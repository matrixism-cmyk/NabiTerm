//! 테스트용 인-프로세스 SSH+SFTP 서버 하네스(러 ssh-sftp 예제 패턴 차용).
//!
//! 외부 서버 없이 SftpFs(list/read/write/remove/rename/mkdir) 왕복을 검증한다.
//! 테스트 본문은 sftp_test.rs.

use russh::server::{self, Auth, Msg, Session};
use russh::{Channel, ChannelId};
use russh_sftp::protocol::{Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) const SERVER_KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYgAAAJgAIAxdACAM
XQAAAAtzc2gtZWQyNTUxOQAAACCzPq7zfqLffKoBDe/eo04kH2XxtSmk9D7RQyf1xUqrYg
AAAEC2BsIi0QwW2uFscKTUUXNHLsYX4FxlaSDSblbAj7WR7bM+rvN+ot98qgEN796jTiQf
ZfG1KaT0PtFDJ/XFSqtiAAAAEHVzZXJAZXhhbXBsZS5jb20BAgMEBQ==
-----END OPENSSH PRIVATE KEY-----
";

#[derive(Default)]
pub(crate) struct SshSession {
    clients: Arc<Mutex<HashMap<ChannelId, Channel<Msg>>>>,
    /// true면 OpenSSH 확장을 하나도 광고하지 않는다(옛 서버 = SFTP v3 순정 흉내).
    pub(crate) bare: bool,
}

impl SshSession {
    /// 확장을 광고하지 않는 서버 핸들러(옛 OpenSSH 흉내).
    pub(crate) fn bare() -> Self {
        Self { bare: true, ..Default::default() }
    }
}

impl server::Handler for SshSession {
    type Error = russh::Error;

    async fn auth_password(&mut self, _u: &str, _p: &str) -> Result<Auth, Self::Error> {
        Ok(Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _s: &mut Session,
    ) -> Result<(), Self::Error> {
        self.clients.lock().await.insert(channel.id(), channel);
        // 0.62: bool 반환 대신 명시적 accept/reject 핸들.
        reply.accept().await;
        Ok(())
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
            russh_sftp::server::run(channel.into_stream(), Sftp::new(self.bare)).await;
        } else {
            session.channel_failure(id)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct Sftp {
    pub(crate) listed: bool,
    pub(crate) files: HashMap<String, Vec<u8>>,
    /// setstat로 설정된 권한(chmod 라운드트립 검증용).
    pub(crate) perms: HashMap<String, u32>,
    /// 확장 없는 옛 서버 흉내(광고도 처리도 안 한다).
    pub(crate) bare: bool,
}

impl Sftp {
    /// readdir가 돌려주는 foo.txt/bar를 읽을 수 있게 내용을 미리 채운다.
    fn new(bare: bool) -> Self {
        let mut files = HashMap::new();
        files.insert("/foo.txt".to_string(), b"foo".to_vec());
        files.insert("/bar".to_string(), b"bar".to_vec());
        Self {
            listed: false,
            files,
            perms: HashMap::new(),
            bare,
        }
    }
}

/// 한 응답에 실제로 담아 주는 최대 바이트. limits로 광고하는 8KB보다 일부러 작게 둔다
/// — 실 서버도 "허용 255KB, 실제 100KB"처럼 다르게 답한다.
pub(crate) const SHORT_READ_CAP: usize = 3000;

pub(crate) fn ok_status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".to_string(),
        language_tag: "en-US".to_string(),
    }
}

impl russh_sftp::server::Handler for Sftp {
    type Error = StatusCode;

    async fn init(
        &mut self,
        _version: u32,
        _ext: std::collections::HashMap<String, String>,
    ) -> Result<russh_sftp::protocol::Version, Self::Error> {
        // OpenSSH 확장을 광고해 클라이언트의 확장 경로가 실제로 돌게 한다.
        // bare 모드에서는 하나도 광고하지 않아 **확장 없는 옛 서버** 경로를 검증한다
        // (사용자 서버가 OpenSSH 4.3 = 순정 v3라 이 갈래가 실제로 쓰인다).
        let extensions =
            if self.bare { Default::default() } else { crate::sftp_serverext::advertised() };
        Ok(russh_sftp::protocol::Version { version: 3, extensions })
    }

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
        // TRUNCATE일 때만 비운다 — 이어올리기(WRITE|CREATE)는 기존 내용을 유지해야 한다.
        if pflags.contains(OpenFlags::TRUNCATE) {
            self.files.insert(filename.clone(), Vec::new());
        } else if pflags.contains(OpenFlags::WRITE) {
            self.files.entry(filename.clone()).or_default();
        } else if !self.files.contains_key(&filename) {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(Handle { id, handle: filename })
    }

    /// 크기를 지정하면 자른다/늘린다(이어올리기의 꼬리 절단 검증용).
    async fn fsetstat(
        &mut self,
        id: u32,
        handle: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        if let Some(sz) = attrs.size {
            if let Some(buf) = self.files.get_mut(&handle) {
                buf.resize(sz as usize, 0);
            }
        }
        Ok(ok_status(id))
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<russh_sftp::protocol::Attrs, Self::Error> {
        let buf = self.files.get(&path).ok_or(StatusCode::NoSuchFile)?;
        let attrs = FileAttributes {
            size: Some(buf.len() as u64),
            permissions: Some(self.perms.get(&path).copied().unwrap_or(0o100_644)),
            mtime: Some(1_700_000_000),
            ..Default::default()
        };
        Ok(russh_sftp::protocol::Attrs { id, attrs })
    }

    async fn extended(
        &mut self,
        id: u32,
        request: String,
        data: Vec<u8>,
    ) -> Result<russh_sftp::protocol::Packet, Self::Error> {
        crate::sftp_serverext::handle(self, id, &request, &data)
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
        // 실 서버(OpenSSH)처럼 요청보다 **짧게** 응답한다. 한도로 알려준 값과 실제로 채워 주는
        // 길이가 다른 상황을 그대로 재현해, 클라이언트가 그 길이에 맞춰 가는지 검증한다.
        let end = (off + (len as usize).min(SHORT_READ_CAP)).min(content.len());
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

