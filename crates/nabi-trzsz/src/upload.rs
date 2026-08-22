//! 업로드 쪽 걸음 — 우리가 보내고 원격이 `#SUCC`로 하나씩 확인한다.
//!
//! v1은 청크마다 왕복이라 느리다. 대신 상태가 단순해서 어디서 끊겨도 무슨 일이 있었는지 안다.
//! 청크 크기는 성공할 때마다 두 배로 키운다 — 느린 링크에서는 작게 시작해 붙잡히지 않고,
//! 빠른 링크에서는 몇 번 만에 상한에 닿는다(시계를 보지 않아도 되는 게 요령이다).

use crate::session::{Session, Step, UlState};
use crate::{decode_payload, encode_payload};

impl Session {
    /// CFG를 받은 뒤 첫 걸음 — 보낼 파일 개수를 알린다.
    pub(crate) fn start_upload(&mut self, out: &mut Vec<Step>) -> Result<(), String> {
        // 폴더를 보내려는데 원격이 폴더 모드가 아니면 **시작하기 전에** 멈춘다.
        // 그냥 보내면 원격이 디렉터리 항목 뒤에 `#SIZE`를 기다리다 프로토콜이 어긋난다
        // (실서버 검증 2026-08-22에서 파이썬 trz가 정확히 그 자리에서 죽었다).
        if !self.cfg.directory && self.items.iter().any(|i| i.entry.is_dir) {
            return Err("the remote is not accepting folders — run `trz -d` there".into());
        }
        self.count = self.items.len();
        out.push(self.send("NUM", &self.count.to_string()));
        self.set_state_upload(UlState::Num);
        Ok(())
    }

    pub(crate) fn on_upload(
        &mut self,
        typ: &str,
        payload: &str,
        out: &mut Vec<Step>,
    ) -> Result<(), String> {
        if typ == "EXIT" {
            return Ok(());
        }
        let Some(state) = self.upload_state() else { return Ok(()) };
        if typ != "SUCC" {
            return Err(format!("unexpected {typ} while uploading"));
        }
        match state {
            UlState::Num => {
                expect_int(payload, self.count as u64)?;
                if self.count == 0 {
                    self.finish_all(out);
                    return Ok(());
                }
                self.send_name(out)
            }
            UlState::Name => {
                // 원격이 실제로 저장한 이름을 돌려준다 — 겹쳐서 바뀌었을 수 있다.
                let raw = decode_payload(payload).ok_or("bad SUCC name")?;
                let remote = String::from_utf8_lossy(&raw).into_owned();
                if !self.names.iter().any(|n| n == &remote) {
                    self.names.push(remote);
                }
                // 디렉터리는 알리기만 하면 끝이다 — 크기도 내용도 없다.
                if self.items.get(self.index).is_some_and(|i| i.entry.is_dir) {
                    return self.next_item(out);
                }
                out.push(self.send("SIZE", &self.size.to_string()));
                self.set_state_upload(UlState::Size);
                Ok(())
            }
            UlState::Size => {
                expect_int(payload, self.size)?;
                out.push(self.progress());
                if self.size == 0 {
                    self.send_md5(out)
                } else {
                    self.send_chunk(out)
                }
            }
            UlState::Data => {
                expect_int(payload, self.last_len)?;
                self.done += self.last_len;
                out.push(self.progress());
                // 잘 갔으니 다음 청크는 두 배로. 상한은 원격이 알려준 bufsize.
                self.chunk = (self.chunk * 2).min(self.cfg.chunk_max());
                if self.done >= self.size {
                    self.send_md5(out)
                } else {
                    self.send_chunk(out)
                }
            }
            UlState::Md5 => {
                let theirs = decode_payload(payload).ok_or("bad SUCC md5")?;
                let ours = self.digest();
                if theirs != ours {
                    return Err(format!("checksum mismatch on {}", self.name));
                }
                self.next_item(out)
            }
        }
    }

    /// 항목 하나를 끝냈다 — 다음 이름을 보내거나 마무리한다.
    fn next_item(&mut self, out: &mut Vec<Step>) -> Result<(), String> {
        self.index += 1;
        if self.index >= self.count {
            self.finish_all(out);
            Ok(())
        } else {
            self.send_name(out)
        }
    }

    /// 다음 항목의 이름을 보낸다(폴더 전송이면 경로 조각이 실린 JSON).
    fn send_name(&mut self, out: &mut Vec<Step>) -> Result<(), String> {
        let item = self.items.get(self.index).ok_or("no such file")?;
        let wire = item.entry.wire_name(self.cfg.directory);
        self.name = item.entry.name().to_owned();
        self.size = item.entry.size;
        self.done = 0;
        self.chunk = crate::action::CHUNK_START;
        self.reset_digest();
        out.push(self.send("NAME", &encode_payload(wire.as_bytes())));
        self.set_state_upload(UlState::Name);
        Ok(())
    }

    /// 다음 데이터 조각을 읽어 보낸다.
    fn send_chunk(&mut self, out: &mut Vec<Step>) -> Result<(), String> {
        let want = self.chunk.min((self.size - self.done) as usize);
        let mut buf = vec![0u8; want];
        let item = self.items.get_mut(self.index).ok_or("no such file")?;
        let src = item.source.as_mut().ok_or("this entry has no content to send")?;
        let n = src.read(&mut buf)?;
        if n == 0 {
            return Err(format!("{} ended early at {} of {}", self.name, self.done, self.size));
        }
        buf.truncate(n);
        self.feed_digest(&buf);
        self.last_len = n as u64;
        out.push(self.send("DATA", &encode_payload(&buf)));
        self.set_state_upload(UlState::Data);
        Ok(())
    }

    fn send_md5(&mut self, out: &mut Vec<Step>) -> Result<(), String> {
        let digest = self.finish_digest();
        out.push(self.send("MD5", &encode_payload(&digest)));
        self.set_state_upload(UlState::Md5);
        Ok(())
    }
}

/// `#SUCC`로 돌아온 정수가 우리가 보낸 값과 같은지. 다르면 어디선가 바이트가 샌 것이다.
fn expect_int(payload: &str, want: u64) -> Result<(), String> {
    let got: u64 = payload.trim().parse().map_err(|_| format!("bad SUCC '{payload}'"))?;
    if got != want {
        return Err(format!("remote acked {got}, expected {want}"));
    }
    Ok(())
}
