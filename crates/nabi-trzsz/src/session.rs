//! 전송 세션 — 트리거를 잡은 뒤 끝까지 가는 상태기계(프로토콜 v1).
//!
//! 파일 입출력은 **호출자가 준 트레이트**로 나간다. 그래서 이 파일은 디스크 없이 시험된다
//! (SFTP에서 배운 것: 코어가 순수해야 회귀를 잡는다. 실서버 검증은 그것대로 따로 한다).

use crate::action::{Action, Config, CHUNK_START};
use crate::line::{render, LineFramer};
use crate::progress::Progress;
use crate::{decode_payload, encode_payload, Mode, Trigger};

/// 우리가 받은 파일을 어디에 어떻게 쓸지 — 호출자(앱)가 구현한다.
pub trait Storage {
    /// 원격 이름으로 로컬 파일을 만든다. 돌려주는 문자열은 **실제 저장 이름**이다
    /// (겹치면 바꿔 붙이므로 원격 이름과 다를 수 있다). 경로 안전 검사도 여기서 한다.
    fn create(&mut self, remote_name: &str, size: u64) -> Result<(String, Box<dyn FileSink>), String>;
}

/// 받은 바이트를 흘려 넣는 곳.
pub trait FileSink {
    fn write(&mut self, data: &[u8]) -> Result<(), String>;
    /// 끝맺음. `ok`가 false면 **받다 만 파일을 지운다**(반쪽 파일을 남기지 않는다).
    fn finish(&mut self, ok: bool) -> Result<(), String>;
}

/// 우리가 보낼 파일에서 바이트를 읽는 곳.
pub trait FileSource {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, String>;
}

/// 보낼 파일 하나.
pub struct UploadItem {
    pub name: String,
    pub size: u64,
    pub source: Box<dyn FileSource>,
}

/// 사용자가 정한 이번 전송의 처리 방침.
pub enum Plan {
    /// 거절한다 — 원격에 `confirm:false`만 알리고 끝낸다.
    Reject(String),
    Download(Box<dyn Storage>),
    Upload(Vec<UploadItem>),
}

/// 세션이 바깥에 요구하는 일.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// 이 바이트를 pane(원격)으로 보내라.
    Write(Vec<u8>),
    Progress(Progress),
    /// 정상 종료 — 요약문과 처리된 이름들.
    Done { summary: String, names: Vec<String> },
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum St {
    WaitConfig,
    DlNum,
    DlName,
    DlSize,
    DlData,
    DlMd5,
    UlNumAck,
    UlNameAck,
    UlSizeAck,
    UlDataAck,
    UlMd5Ack,
    Ended,
}

pub struct Session {
    pub(crate) mode: Mode,
    framer: LineFramer,
    pub(crate) cfg: Config,
    pub(crate) newline: String,
    state: St,
    storage: Option<Box<dyn Storage>>,
    sink: Option<Box<dyn FileSink>>,
    pub(crate) items: Vec<UploadItem>,
    hasher: md5::Context,
    pub(crate) count: usize,
    pub(crate) index: usize,
    pub(crate) name: String,
    pub(crate) size: u64,
    pub(crate) done: u64,
    pub(crate) names: Vec<String>,
    pub(crate) chunk: usize,
    pub(crate) last_len: u64,
    /// 업로드에서 마지막에 계산한 MD5 — `#MD5`를 보낼 때 만들고 `#SUCC` 대조에 다시 쓴다
    /// (`Context::finalize`가 값을 먹어 버려 두 번 계산할 수 없다).
    digest: [u8; 16],
}

impl Session {
    /// 트리거를 받아 세션을 연다. 첫 걸음은 언제나 `#ACT` 전송이다.
    pub fn new(trigger: &Trigger, plan: Plan) -> (Self, Vec<Step>) {
        let newline = if trigger.win_server { "!\n" } else { "\n" };
        let reject = matches!(plan, Plan::Reject(_));
        let (storage, items) = match plan {
            Plan::Download(s) => (Some(s), Vec::new()),
            Plan::Upload(v) => (None, v),
            Plan::Reject(_) => (None, Vec::new()),
        };
        let s = Self {
            mode: trigger.mode,
            framer: LineFramer::new(),
            cfg: Config::default(),
            newline: newline.to_owned(),
            state: if reject { St::Ended } else { St::WaitConfig },
            storage,
            sink: None,
            items,
            hasher: md5::Context::new(),
            count: 0,
            index: 0,
            name: String::new(),
            size: 0,
            done: 0,
            names: Vec::new(),
            chunk: CHUNK_START,
            last_len: 0,
            digest: [0; 16],
        };
        let act = Action::new(!reject, trigger.win_server);
        let mut steps = vec![s.send("ACT", &encode_payload(json(&act).as_bytes()))];
        if reject {
            steps.push(Step::Done { summary: String::new(), names: Vec::new() });
        }
        (s, steps)
    }

    /// 원격이 보낸 바이트를 먹인다.
    pub fn on_bytes(&mut self, chunk: &[u8]) -> Vec<Step> {
        if self.state == St::Ended {
            return Vec::new();
        }
        let mut out = Vec::new();
        for line in self.framer.feed(chunk) {
            if line.is_failure() {
                let why = decode_payload(&line.payload)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_else(|| line.payload.clone());
                return self.fail(&mut out, why);
            }
            match self.step(&line.typ, &line.payload, &mut out) {
                Ok(()) => {}
                Err(why) => return self.fail(&mut out, why),
            }
            if self.state == St::Ended {
                break;
            }
        }
        if self.framer.had_overflow() {
            return self.fail(&mut out, "protocol line too long".into());
        }
        out
    }

    /// 사용자가 취소했다 — 원격에 알리고 받다 만 파일을 지운다.
    pub fn cancel(&mut self) -> Vec<Step> {
        let mut out = Vec::new();
        self.fail(&mut out, "cancelled by user".into())
    }

    pub fn is_ended(&self) -> bool {
        self.state == St::Ended
    }

    fn fail(&mut self, out: &mut Vec<Step>, why: String) -> Vec<Step> {
        if self.state != St::Ended {
            // 소문자 `fail`은 "스택 추적 없이 조용히"라는 뜻이다(원격이 그렇게 읽는다).
            out.push(self.send("fail", &encode_payload(why.as_bytes())));
            self.close_sink(false);
            self.state = St::Ended;
            out.push(Step::Failed(why));
        }
        std::mem::take(out)
    }

    pub(crate) fn send(&self, typ: &str, payload: &str) -> Step {
        Step::Write(render(typ, payload, &self.newline))
    }

    pub(crate) fn progress(&self) -> Step {
        Step::Progress(Progress {
            index: self.index + 1,
            count: self.count,
            name: self.name.clone(),
            done: self.done,
            total: self.size,
            bps: 0, // 속도는 UI 쪽 Rate가 시계를 보고 채운다.
        })
    }

    pub(crate) fn finish_all(&mut self, out: &mut Vec<Step>) {
        let summary = format!("{} file(s)", self.names.len());
        out.push(self.send("EXIT", &encode_payload(summary.as_bytes())));
        out.push(Step::Done { summary, names: std::mem::take(&mut self.names) });
        self.state = St::Ended;
    }

    fn close_sink(&mut self, ok: bool) {
        if let Some(mut s) = self.sink.take() {
            let _ = s.finish(ok);
        }
    }

    /// 프레임 하나를 처리한다.
    fn step(&mut self, typ: &str, payload: &str, out: &mut Vec<Step>) -> Result<(), String> {
        match self.state {
            St::WaitConfig => self.on_config(typ, payload, out),
            St::DlNum | St::DlName | St::DlSize | St::DlData | St::DlMd5 => {
                self.on_download(typ, payload, out)
            }
            _ => self.on_upload(typ, payload, out),
        }
    }

    fn on_config(&mut self, typ: &str, payload: &str, out: &mut Vec<Step>) -> Result<(), String> {
        if typ != "CFG" {
            return Ok(()); // 설정 전 잡음은 흘린다(프롬프트 잔여물 등).
        }
        let raw = decode_payload(payload).ok_or("bad CFG payload")?;
        let cfg: Config = serde_json::from_slice(&raw).map_err(|e| format!("bad CFG json: {e}"))?;
        self.cfg = cfg.sanitized();
        // 원격이 줄바꿈을 명시했을 때만 바꾼다. 명시하지 않으면 `#ACT`에서 정한 값이 맞다.
        if let Some(nl) = self.cfg.newline.clone() {
            self.newline = nl;
        }
        if self.cfg.protocol != 1 {
            return Err(format!("unsupported protocol v{}", self.cfg.protocol));
        }
        if self.cfg.binary {
            return Err("binary mode not supported yet".into());
        }
        if self.mode.is_upload() {
            self.start_upload(out)
        } else {
            self.state = St::DlNum;
            Ok(())
        }
    }

    // ─── 다운로드(원격이 보낸다) ───────────────────────────────────────────────
    fn on_download(&mut self, typ: &str, payload: &str, out: &mut Vec<Step>) -> Result<(), String> {
        match (self.state, typ) {
            (St::DlNum, "NUM") => {
                self.count = payload.parse().map_err(|_| "bad NUM")?;
                out.push(self.send("SUCC", payload));
                if self.count == 0 {
                    self.finish_all(out);
                } else {
                    self.state = St::DlName;
                }
                Ok(())
            }
            (St::DlName, "NAME") => {
                let raw = decode_payload(payload).ok_or("bad NAME")?;
                self.name = String::from_utf8_lossy(&raw).into_owned();
                let st = self.storage.as_mut().ok_or("no storage")?;
                let (local, sink) = st.create(&self.name, 0)?;
                self.sink = Some(sink);
                self.names.push(local.clone());
                out.push(self.send("SUCC", &encode_payload(local.as_bytes())));
                self.state = St::DlSize;
                Ok(())
            }
            (St::DlSize, "SIZE") => {
                self.size = payload.parse().map_err(|_| "bad SIZE")?;
                self.done = 0;
                self.hasher = md5::Context::new();
                out.push(self.send("SUCC", payload));
                out.push(self.progress());
                self.state = if self.size == 0 { St::DlMd5 } else { St::DlData };
                Ok(())
            }
            (St::DlData, "DATA") => {
                let data = decode_payload(payload).ok_or("bad DATA")?;
                if self.done + data.len() as u64 > self.size {
                    return Err("remote sent more than the declared size".into());
                }
                self.hasher.consume(&data);
                self.sink.as_mut().ok_or("no sink")?.write(&data)?;
                self.done += data.len() as u64;
                out.push(self.send("SUCC", &data.len().to_string()));
                out.push(self.progress());
                if self.done >= self.size {
                    self.state = St::DlMd5;
                }
                Ok(())
            }
            (St::DlMd5, "MD5") => {
                let theirs = decode_payload(payload).ok_or("bad MD5")?;
                let ours = std::mem::replace(&mut self.hasher, md5::Context::new()).finalize();
                if theirs != ours.0 {
                    self.close_sink(false);
                    return Err(format!("checksum mismatch on {}", self.name));
                }
                self.close_sink(true);
                out.push(self.send("SUCC", &encode_payload(&ours.0)));
                self.index += 1;
                if self.index >= self.count {
                    self.finish_all(out);
                } else {
                    self.state = St::DlName;
                }
                Ok(())
            }
            (_, "EXIT") => Ok(()),
            (st, t) => Err(format!("unexpected {t} in {st:?}")),
        }
    }

    pub(crate) fn set_state_upload(&mut self, s: UlState) {
        self.state = match s {
            UlState::Num => St::UlNumAck,
            UlState::Name => St::UlNameAck,
            UlState::Size => St::UlSizeAck,
            UlState::Data => St::UlDataAck,
            UlState::Md5 => St::UlMd5Ack,
        };
    }

    /// 새 파일을 시작하며 해시를 비운다.
    pub(crate) fn reset_digest(&mut self) {
        self.hasher = md5::Context::new();
        self.digest = [0; 16];
    }

    pub(crate) fn feed_digest(&mut self, data: &[u8]) {
        self.hasher.consume(data);
    }

    /// 지금까지 먹인 것으로 MD5를 확정한다(업로드에서 `#MD5`를 보내기 직전).
    pub(crate) fn finish_digest(&mut self) -> [u8; 16] {
        self.digest = std::mem::replace(&mut self.hasher, md5::Context::new()).finalize().0;
        self.digest
    }

    /// 확정해 둔 MD5(원격의 `#SUCC` 대조용).
    pub(crate) fn digest(&self) -> [u8; 16] {
        self.digest
    }

    pub(crate) fn upload_state(&self) -> Option<UlState> {
        Some(match self.state {
            St::UlNumAck => UlState::Num,
            St::UlNameAck => UlState::Name,
            St::UlSizeAck => UlState::Size,
            St::UlDataAck => UlState::Data,
            St::UlMd5Ack => UlState::Md5,
            _ => return None,
        })
    }
}

/// 업로드 쪽 단계(upload.rs가 쓴다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UlState {
    Num,
    Name,
    Size,
    Data,
    Md5,
}

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
}
