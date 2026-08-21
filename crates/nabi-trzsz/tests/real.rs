//! **실제 trzsz 구현**을 상대로 하는 검증 — 기본은 건너뛰고(`#[ignore]`) 명시할 때만 돈다.
//!
//! 왜 이게 따로 필요한가: 우리가 만든 가짜 원격은 **우리가 기대한 대로만** 답한다.
//! SFTP에서 두 번 다 실서버에서만 결함이 나왔다. 여기서는 파이썬 구현(trzsz-svr)의
//! `trz`/`tsz`를 자식 프로세스로 띄워 표준입출력으로 진짜 프로토콜을 주고받는다.
//!
//! ```text
//! pip install trzsz
//! set NABI_TRZSZ_BIN=C:\Program Files\Python313\Scripts   (또는 PATH에 두기)
//! cargo test -p nabi-trzsz --test real -- --ignored --nocapture
//! ```

mod harness;

use harness::{MemSource, MemStorage};
use nabi_trzsz::{Plan, Session, Step, TriggerScanner, UploadItem};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// `trz`/`tsz` 실행 파일 경로. `NABI_TRZSZ_BIN`이 있으면 그 폴더에서 찾는다.
fn tool(name: &str) -> PathBuf {
    match std::env::var_os("NABI_TRZSZ_BIN") {
        Some(dir) => PathBuf::from(dir).join(format!("{name}.exe")),
        None => PathBuf::from(name),
    }
}

/// `NABI_TRZSZ_TRACE=1`이면 오간 프레임을 찍는다 — 실서버 결함은 이 기록으로 찾는다.
/// 줄바꿈 협상 버그도 이 출력에서 원격과 우리 줄 끝이 다른 것으로 드러났다.
fn trace(dir: &str, b: &[u8]) {
    if std::env::var_os("NABI_TRZSZ_TRACE").is_some() {
        eprintln!("{dir} {:?}", String::from_utf8_lossy(&b[..b.len().min(160)]));
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nabi-trzsz-real-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    d
}

/// 자식의 stdout을 별도 스레드에서 읽어 채널로 넘긴다(파이프가 막히지 않게).
fn reader(child: &mut Child) -> mpsc::Receiver<Vec<u8>> {
    let mut out = child.stdout.take().expect("stdout");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    rx
}

/// 세션을 실제 자식과 끝까지 주고받게 한다. (완료 이름들, 실패 사유)
fn pump(child: &mut Child, mut s: Session, first: Vec<Step>, rest: &[u8]) -> (Vec<String>, Option<String>) {
    let rx = reader(child);
    let mut stdin = child.stdin.take().expect("stdin");
    let (mut names, mut failure) = (Vec::new(), None);
    let mut steps = first;
    if !rest.is_empty() {
        steps.extend(s.on_bytes(rest));
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        for step in steps.drain(..) {
            match step {
                Step::Write(b) => {
                    trace(">>", &b);
                    stdin.write_all(&b).expect("write to child");
                    stdin.flush().ok();
                }
                Step::Done { names: n, .. } => names = n,
                Step::Failed(e) => failure = Some(e),
                Step::Progress(_) => {}
            }
        }
        if s.is_ended() {
            break;
        }
        match rx.recv_timeout(Duration::from_secs(20)) {
            Ok(chunk) => {
                trace("<<", &chunk);
                steps = s.on_bytes(&chunk);
            }
            Err(_) => {
                failure = Some("timed out waiting for the real trzsz".into());
                break;
            }
        }
    }
    (names, failure)
}

/// 자식이 낸 첫 출력에서 트리거를 찾는다. (트리거, 트리거 뒤 남은 바이트)
fn wait_trigger(child: &mut Child) -> (nabi_trzsz::Trigger, Vec<u8>) {
    // 트리거는 첫 줄에 나온다. 읽기 스레드를 쓰기 전이라 여기서만 직접 읽는다.
    let out = child.stdout.as_mut().expect("stdout");
    let mut scanner = TriggerScanner::new();
    let mut buf = [0u8; 4096];
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        let n = out.read(&mut buf).expect("read child");
        assert!(n > 0, "자식이 트리거 없이 끝났다");
        let scanned = scanner.feed(&buf[..n]);
        if let Some(t) = scanned.trigger {
            return (t, scanned.rest);
        }
    }
    panic!("트리거가 오지 않았다");
}

fn spawn(name: &str, args: &[&str], cwd: &PathBuf) -> Option<Child> {
    Command::new(tool(name))
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .ok()
}

#[test]
#[ignore = "실제 trzsz 설치 필요(pip install trzsz)"]
fn receives_a_real_file_from_tsz() {
    let dir = tmp_dir("dl");
    let src = dir.join("payload.bin");
    // 압축이 잘 되는 구간과 안 되는 구간을 섞는다 — zlib 경로를 양쪽 다 지난다.
    let mut data: Vec<u8> = vec![0u8; 20_000];
    data.extend((0..30_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8));
    std::fs::write(&src, &data).unwrap();

    let Some(mut child) = spawn("tsz", &["-q", src.to_str().unwrap()], &dir) else {
        eprintln!("tsz 를 찾지 못했다 — 건너뛴다");
        return;
    };
    let (trigger, rest) = wait_trigger(&mut child);
    assert!(!trigger.mode.is_upload(), "tsz 는 우리가 받는 쪽이어야 한다");

    let store = MemStorage::new();
    let files = store.shared();
    let (session, first) = Session::new(&trigger, Plan::Download(Box::new(store)));
    let (names, err) = pump(&mut child, session, first, &rest);
    let _ = child.kill();

    assert_eq!(err, None, "실서버 다운로드가 실패했다");
    assert_eq!(names, vec!["payload.bin".to_string()]);
    let got = files.borrow();
    assert_eq!(got[0].1.len(), data.len(), "받은 크기가 다르다");
    assert_eq!(got[0].1, data, "받은 내용이 다르다");
    assert!(got[0].2, "정상 종료 표시가 없다");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "실제 trzsz 설치 필요(pip install trzsz)"]
fn sends_a_real_file_to_trz() {
    let dir = tmp_dir("ul");
    let data: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();

    let Some(mut child) = spawn("trz", &["-q", "-y", dir.to_str().unwrap()], &dir) else {
        eprintln!("trz 를 찾지 못했다 — 건너뛴다");
        return;
    };
    let (trigger, rest) = wait_trigger(&mut child);
    assert!(trigger.mode.is_upload(), "trz 는 우리가 보내는 쪽이어야 한다");

    let item = UploadItem {
        name: "sent.bin".into(),
        size: data.len() as u64,
        source: Box::new(MemSource::new(data.clone())),
    };
    let (session, first) = Session::new(&trigger, Plan::Upload(vec![item]));
    let (names, err) = pump(&mut child, session, first, &rest);
    child.wait_timeout();

    assert_eq!(err, None, "실서버 업로드가 실패했다");
    assert_eq!(names, vec!["sent.bin".to_string()], "원격이 저장한 이름");
    let landed = std::fs::read(dir.join("sent.bin")).expect("원격이 파일을 만들지 않았다");
    assert_eq!(landed, data, "올린 내용이 다르다");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `Child`가 끝나기를 잠깐 기다린다(파일이 디스크에 닿을 시간).
trait WaitTimeout {
    fn wait_timeout(&mut self);
}

impl WaitTimeout for Child {
    fn wait_timeout(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match self.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => return,
            }
        }
        let _ = self.kill();
    }
}
