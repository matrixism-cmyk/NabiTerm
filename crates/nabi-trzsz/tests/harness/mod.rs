//! 흐름 시험용 가짜 원격과 메모리 저장소.
//!
//! `real.rs`는 이 중 일부만 쓴다(진짜 원격이 상대라 가짜 원격이 필요 없다).
#![allow(dead_code)]

use nabi_trzsz::{
    decode_payload, encode_payload, render, Entry, FileSink, FileSource, LineFramer, Session, Step,
    Storage, Trigger, TriggerScanner,
};
use std::cell::RefCell;
use std::rc::Rc;

/// (원격이 준 이름, 내용, 정상 종료했는가)
pub type Files = Rc<RefCell<Vec<(String, Vec<u8>, bool)>>>;

pub struct MemStorage {
    files: Files,
    dirs: Rc<RefCell<Vec<String>>>,
}

impl MemStorage {
    pub fn new() -> Self {
        Self { files: Rc::new(RefCell::new(Vec::new())), dirs: Rc::new(RefCell::new(Vec::new())) }
    }
    pub fn shared(&self) -> Files {
        self.files.clone()
    }
    /// 만들어진 디렉터리 목록(폴더 전송 시험).
    pub fn dirs(&self) -> Rc<RefCell<Vec<String>>> {
        self.dirs.clone()
    }
}

struct MemSink {
    files: Files,
    idx: usize,
}

impl Storage for MemStorage {
    fn create(&mut self, entry: &Entry) -> Result<(String, Option<Box<dyn FileSink>>), String> {
        // 진짜 구현의 경로 검사를 흉내 낸다 — 이 시험의 핵심 하나가 여기다.
        for part in &entry.rel {
            if part.contains("..") || part.contains('/') || part.contains('\\') {
                return Err(format!("path traversal refused: {part}"));
            }
        }
        let full = entry.rel.join("/");
        if entry.is_dir {
            self.dirs.borrow_mut().push(full);
            return Ok((entry.root().to_owned(), None));
        }
        self.files.borrow_mut().push((full, Vec::new(), false));
        let idx = self.files.borrow().len() - 1;
        Ok((entry.root().to_owned(), Some(Box::new(MemSink { files: self.files.clone(), idx }))))
    }
}

impl FileSink for MemSink {
    fn write(&mut self, data: &[u8]) -> Result<(), String> {
        self.files.borrow_mut()[self.idx].1.extend_from_slice(data);
        Ok(())
    }
    fn finish(&mut self, ok: bool) -> Result<(), String> {
        self.files.borrow_mut()[self.idx].2 = ok;
        Ok(())
    }
}

pub struct MemSource {
    data: Vec<u8>,
    pos: usize,
}

impl MemSource {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }
}

impl FileSource for MemSource {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String> {
        let n = buf.len().min(self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// 원격 역할. 보내는 쪽(`tsz`)이거나 받는 쪽(`trz`)이다.
pub struct Remote {
    framer: LineFramer,
    files: Vec<(String, Vec<u8>)>,
    idx: usize,
    stage: u8,
    sent: usize,
    got: Vec<(String, Vec<u8>)>,
    sending: bool,
    pub saw_exit: bool,
    pub saw_confirm: bool,
    /// MD5를 일부러 틀리게 보낸다(무결성 검사 시험).
    pub corrupt_md5: bool,
    /// CFG 직후 실패를 통보한다.
    pub fail_after_cfg: bool,
    /// 선언한 SIZE보다 많이 보낸다(디스크 채우기 시험).
    pub oversend: bool,
    /// CFG에 directory:true 를 실어 `#NAME`을 JSON으로 주고받는다.
    directory: bool,
}

impl Remote {
    pub fn sender(files: Vec<(String, Vec<u8>)>) -> Self {
        Self::make(true, files)
    }
    pub fn receiver() -> Self {
        Self::make(false, Vec::new())
    }
    fn make(sending: bool, files: Vec<(String, Vec<u8>)>) -> Self {
        Self {
            framer: LineFramer::new(),
            files,
            idx: 0,
            stage: 0,
            sent: 0,
            got: Vec::new(),
            sending,
            saw_exit: false,
            saw_confirm: false,
            corrupt_md5: false,
            fail_after_cfg: false,
            oversend: false,
            directory: false,
        }
    }

    /// 우리가 받은 파일들(업로드 시험).
    pub fn received(&self) -> Vec<(String, Vec<u8>)> {
        self.got.clone()
    }

    /// 클라이언트가 보낸 바이트에 답한다.
    pub fn reply(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for line in self.framer.feed(bytes) {
            let p = line.payload.clone();
            match line.typ.as_str() {
                "ACT" => out.extend(self.on_act(&p)),
                "NUM" => out.extend(l("SUCC", &p)),
                "NAME" => {
                    let n = decode_payload(&p).expect("NAME");
                    let text = String::from_utf8_lossy(&n).into_owned();
                    // 진짜 원격은 저장한 **최상위 이름**만 돌려준다(폴더 전송에서 특히 중요 —
                    // JSON을 그대로 되돌리면 항목마다 다른 이름이 되어 목록이 부풀어 오른다).
                    let saved = Entry::parse(&text, self.directory)
                        .map_or_else(|_| text.clone(), |e| e.root().to_owned());
                    self.got.push((text, Vec::new()));
                    out.extend(l("SUCC", &encode_payload(saved.as_bytes())));
                }
                "SIZE" => out.extend(l("SUCC", &p)),
                "DATA" => {
                    let d = decode_payload(&p).expect("DATA");
                    if let Some(f) = self.got.last_mut() {
                        f.1.extend_from_slice(&d);
                    }
                    out.extend(l("SUCC", &d.len().to_string()));
                }
                "MD5" => out.extend(l("SUCC", &p)),
                "SUCC" => out.extend(self.on_succ()),
                "EXIT" | "FAIL" | "fail" => self.saw_exit = true,
                _ => {}
            }
        }
        out
    }

    /// CFG에 directory:true 를 넣어 폴더 전송을 흉내 낸다.
    pub fn directory_mode(mut self) -> Self {
        self.directory = true;
        self
    }

    fn on_act(&mut self, payload: &str) -> Vec<u8> {
        let act = decode_payload(payload).expect("ACT는 zlib+base64여야 한다");
        let txt = String::from_utf8_lossy(&act).into_owned();
        self.saw_confirm = txt.contains("\"confirm\":true");
        if !self.saw_confirm {
            return Vec::new(); // 거절이면 원격은 조용히 끝난다.
        }
        let cfg = if self.directory {
            r#"{"lang":"go","bufsize":4096,"timeout":100,"protocol":1,"directory":true}"#
        } else {
            r#"{"lang":"go","bufsize":4096,"timeout":100,"protocol":1}"#
        };
        let mut out = l("CFG", &encode_payload(cfg.as_bytes()));
        if self.fail_after_cfg {
            out.extend(l("FAIL", &encode_payload(b"disk full on the remote")));
            return out;
        }
        if self.sending {
            out.extend(l("NUM", &self.files.len().to_string()));
        }
        out
    }

    /// 다운로드에서 우리 확인을 받고 다음 프레임을 보낸다.
    fn on_succ(&mut self) -> Vec<u8> {
        if !self.sending || self.idx >= self.files.len() {
            return Vec::new();
        }
        let data = self.files[self.idx].1.clone();
        match self.stage {
            0 => {
                self.stage = 1;
                self.name_frame(self.idx)
            }
            1 => {
                self.stage = 2;
                self.sent = 0;
                // 크기를 줄여 말하고 실제로는 더 보낸다 — 디스크를 채우는 고전 수법이다.
                let claim = if self.oversend { 1 } else { data.len() };
                l("SIZE", &claim.to_string())
            }
            2 | 3 => {
                if self.sent >= data.len() {
                    self.stage = 4;
                    self.md5_of(&data)
                } else {
                    self.stage = 3;
                    let end = (self.sent + 700).min(data.len());
                    let part = data[self.sent..end].to_vec();
                    self.sent = end;
                    l("DATA", &encode_payload(&part))
                }
            }
            _ => {
                self.idx += 1;
                self.stage = 0;
                if self.idx >= self.files.len() {
                    Vec::new()
                } else {
                    self.stage = 1;
                    self.name_frame(self.idx)
                }
            }
        }
    }

    /// `#NAME` 한 프레임. 디렉터리 모드면 경로를 조각내 JSON으로 싣는다.
    /// 이름이 `/`로 끝나면 디렉터리 항목으로 본다.
    fn name_frame(&mut self, idx: usize) -> Vec<u8> {
        let raw = self.files[idx].0.clone();
        if !self.directory {
            return l("NAME", &encode_payload(raw.as_bytes()));
        }
        let is_dir = raw.ends_with('/');
        let rel: Vec<String> =
            raw.trim_end_matches('/').split('/').map(str::to_owned).collect();
        let e = Entry { path_id: 0, rel, is_dir, size: 0, perm: None };
        if is_dir {
            self.stage = 4; // 디렉터리에는 SIZE·DATA·MD5가 없다 — 다음 이름으로 넘어간다.
        }
        l("NAME", &encode_payload(e.wire_name(true).as_bytes()))
    }

    fn md5_of(&self, data: &[u8]) -> Vec<u8> {
        let mut d = md5::compute(data).0;
        if self.corrupt_md5 {
            d[0] ^= 0xff;
        }
        l("MD5", &encode_payload(&d))
    }
}

fn l(typ: &str, payload: &str) -> Vec<u8> {
    render(typ, payload, "\n")
}

/// 매직 문자열에서 트리거를 만든다.
pub fn trigger(mode: char) -> Trigger {
    let s = format!("::TRZSZ:TRANSFER:{mode}:1.1.8:1755780000000\n");
    TriggerScanner::new().feed(s.as_bytes()).trigger.expect("트리거를 잡아야 한다")
}

/// 걸음에서 보낼 바이트만 뽑는다.
pub fn writes(steps: Vec<Step>) -> Vec<u8> {
    let mut out = Vec::new();
    for s in steps {
        if let Step::Write(b) = s {
            out.extend_from_slice(&b);
        }
    }
    out
}

/// 세션과 원격을 끝까지 주고받게 한다. (완료된 이름들, 실패 사유)
pub fn drive(
    mut session: Session,
    first: Vec<Step>,
    remote: &mut Remote,
) -> (Vec<String>, Option<String>) {
    let (mut names, mut failure) = (Vec::new(), None);
    let mut steps = first;
    // 한 파일이 여러 청크로 나뉘므로 넉넉히 돈다. 무한 루프는 여기서 막힌다.
    for _ in 0..20_000 {
        let mut pending = Vec::new();
        for s in steps.drain(..) {
            match s {
                Step::Write(b) => pending.extend_from_slice(&b),
                Step::Done { names: n, .. } => names = n,
                Step::Failed(e) => failure = Some(e),
                Step::Progress(_) => {}
            }
        }
        let back = remote.reply(&pending);
        if session.is_ended() || back.is_empty() {
            break;
        }
        steps = session.on_bytes(&back);
    }
    (names, failure)
}
