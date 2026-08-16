//! LSP 클라이언트(T6-4 1단계) — 서버 프로세스 + 리더 스레드 + 진단 수신/요청-응답.
//!
//! tokio 없이 std 스레드로 동작한다(에디터 크레이트를 가볍게 유지). UI는 블로킹하지 않고
//! 공유 상태(진단 맵·응답 슬롯)를 프레임마다 읽는다. v1 범위: initialize/didOpen/didChange/
//! publishDiagnostics/definition. 서버가 없거나 죽으면 조용히 비활성(에디터는 평소대로).

use crate::lspframe::{canon_uri, encode, path_to_uri};
use crate::lspread::{parse_definition, reader_loop};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// 진단 하나(0-base 줄/열).
#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub line: u32,
    pub col: u32,
    /// 1=Error 2=Warning 3=Info 4=Hint.
    pub severity: u8,
    pub message: String,
}

/// 정의 위치(경로 + 0-base 줄/열).
#[derive(Clone, Debug)]
pub struct DefLoc {
    pub path: PathBuf,
    pub line: u32,
    pub col: u32,
}

#[derive(Default)]
pub(crate) struct Shared {
    /// URI → 진단 목록(publishDiagnostics가 갱신).
    pub(crate) diags: Mutex<HashMap<String, Vec<Diag>>>,
    /// 요청 id → 응답(리더가 채움; 요청자가 폴링해 꺼냄).
    pub(crate) replies: Mutex<HashMap<i64, Value>>,
}

pub struct LspClient {
    child: Child,
    stdin: Arc<Mutex<std::process::ChildStdin>>,
    shared: Arc<Shared>,
    next_id: AtomicI64,
    ready: Arc<AtomicBool>,
    versions: Mutex<HashMap<String, i64>>,
}

impl LspClient {
    /// 서버를 띄우고 initialize 핸드셰이크를 시작한다(응답은 리더가 ready 플래그로).
    pub fn start(server: &str, root: &Path) -> Option<LspClient> {
        let mut child = Command::new(server)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;
        let shared = Arc::new(Shared::default());
        let ready = Arc::new(AtomicBool::new(false));
        let stdin = Arc::new(Mutex::new(stdin));
        let (sh, wr) = (shared.clone(), stdin.clone());
        std::thread::spawn(move || reader_loop(stdout, sh, wr));
        let c = LspClient {
            child,
            stdin,
            shared,
            next_id: AtomicI64::new(1),
            ready,
            versions: Mutex::new(HashMap::new()),
        };
        let root_uri = path_to_uri(root);
        let id = c.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "capabilities": {
                    "textDocument": {
                        "publishDiagnostics": {},
                        "definition": {},
                        "synchronization": { "didSave": true }
                    }
                },
                "workspaceFolders": [{ "uri": root_uri, "name": "root" }]
            }),
        )?;
        // initialized 통지는 initialize "응답 후"에 보내야 한다 — 응답 폴링 스레드가 마무리
        // (UI 비블로킹; 60초 내 미응답이면 ready로 승격되지 않아 조용히 비활성 유지).
        let sh = c.shared.clone();
        let rd = c.ready.clone();
        let stdin2 = c.stdin.clone();
        std::thread::spawn(move || {
            for _ in 0..600 {
                if sh.replies.lock().ok().map(|m| m.contains_key(&id)).unwrap_or(false) {
                    if let Ok(mut w) = stdin2.lock() {
                        let note = json!({"jsonrpc":"2.0","method":"initialized","params":{}});
                        let _ = w.write_all(&encode(&note.to_string()));
                        let _ = w.flush();
                    }
                    rd.store(true, Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        Some(c)
    }

    /// 서버가 initialize를 마쳤는지(그 전 didOpen은 큐잉 없이 버려질 수 있어 호출측이 확인).
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    fn send(&self, v: &Value) -> Option<()> {
        let mut w = self.stdin.lock().ok()?;
        w.write_all(&encode(&v.to_string())).ok()?;
        w.flush().ok()
    }

    fn request(&self, method: &str, params: Value) -> Option<i64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        Some(id)
    }

    fn notify(&self, method: &str, params: Value) {
        let _ = self.send(&json!({"jsonrpc":"2.0","method":method,"params":params}));
    }

    /// 문서 열림 통지(전체 텍스트).
    pub fn did_open(&self, path: &Path, text: &str) {
        let uri = path_to_uri(path);
        if let Ok(mut v) = self.versions.lock() {
            v.insert(uri.clone(), 1);
        }
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": "rust", "version": 1, "text": text}}),
        );
    }

    /// 문서 변경 통지(전체 동기화 — v1).
    pub fn did_change(&self, path: &Path, text: &str) {
        let uri = path_to_uri(path);
        let ver = self
            .versions
            .lock()
            .ok()
            .map(|mut m| {
                let e = m.entry(uri.clone()).or_insert(1);
                *e += 1;
                *e
            })
            .unwrap_or(2);
        self.notify(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": ver}, "contentChanges": [{"text": text}]}),
        );
    }

    /// 저장 통지 — rust-analyzer는 이때 cargo check(플라이체크)를 돌린다.
    pub fn did_save(&self, path: &Path) {
        self.notify("textDocument/didSave", json!({"textDocument": {"uri": path_to_uri(path)}}));
    }

    /// 문서의 현재 진단(0-base 줄 기준, 심각도 오름차순 정렬).
    pub fn diagnostics(&self, path: &Path) -> Vec<Diag> {
        let uri = canon_uri(&path_to_uri(path));
        let mut v = self
            .shared
            .diags
            .lock()
            .ok()
            .and_then(|m| m.get(&uri).cloned())
            .unwrap_or_default();
        v.sort_by_key(|d| (d.severity, d.line));
        v
    }

    /// 정의로 이동 요청을 보낸다(응답은 `take_definition`으로 폴링).
    pub fn request_definition(&self, path: &Path, line: u32, col: u32) -> Option<i64> {
        self.request(
            "textDocument/definition",
            json!({"textDocument": {"uri": path_to_uri(path)}, "position": {"line": line, "character": col}}),
        )
    }

    /// 정의 응답이 도착했으면 꺼낸다(Location | Location[] | LocationLink[] 수용).
    pub fn take_definition(&self, id: i64) -> Option<Option<DefLoc>> {
        let v = self.shared.replies.lock().ok()?.remove(&id)?;
        Some(parse_definition(&v))
    }

    /// 서버 프로세스가 살아 있는지.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.send(&json!({"jsonrpc":"2.0","id":9_999_999,"method":"shutdown","params":null}));
        let _ = self.send(&json!({"jsonrpc":"2.0","method":"exit","params":null}));
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = self.child.kill();
    }
}
