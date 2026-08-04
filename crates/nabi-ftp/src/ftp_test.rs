//! 인-프로세스 최소 FTP 서버로 FtpFs(FTP 백엔드)를 런타임 검증한다.
//!
//! suppaftp의 list 흐름(USER/PASS/PASV·EPSV/LIST)에 필요한 최소 FTP만 구현.

use crate::connect_ftp;
use nabi_fs::{FileKind, RemoteFs};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

async fn handle(ctrl: TcpStream) {
    let (r, mut w) = ctrl.into_split();
    let mut reader = BufReader::new(r);
    let _ = w.write_all(b"220 nabi-test-ftp\r\n").await;
    let mut data: Option<TcpListener> = None;
    let mut line = String::new();

    loop {
        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            break;
        }
        let up = line.trim_end().to_uppercase();
        let cmd = up.split_whitespace().next().unwrap_or("");
        match cmd {
            "USER" => send(&mut w, b"331 ok\r\n").await,
            "PASS" => send(&mut w, b"230 ok\r\n").await,
            "SYST" => send(&mut w, b"215 UNIX Type: L8\r\n").await,
            "FEAT" => send(&mut w, b"211-Feat\r\n211 End\r\n").await,
            "PWD" | "XPWD" => send(&mut w, b"257 \"/\"\r\n").await,
            "CWD" => send(&mut w, b"250 ok\r\n").await,
            "PASV" => {
                let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let p = l.local_addr().unwrap().port();
                data = Some(l);
                let msg =
                    format!("227 Entering Passive Mode (127,0,0,1,{},{})\r\n", p >> 8, p & 0xff);
                send(&mut w, msg.as_bytes()).await;
            }
            "EPSV" => {
                let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let p = l.local_addr().unwrap().port();
                data = Some(l);
                let msg = format!("229 Entering Extended Passive Mode (|||{p}|)\r\n");
                send(&mut w, msg.as_bytes()).await;
            }
            "LIST" | "NLST" => {
                send(&mut w, b"150 here\r\n").await;
                if let Some(l) = data.take() {
                    if let Ok((mut d, _)) = l.accept().await {
                        let _ = d
                            .write_all(b"-rw-r--r-- 1 u g 1234 Jan 1 00:00 alpha.txt\r\n")
                            .await;
                        let _ = d
                            .write_all(b"drwxr-xr-x 2 u g 4096 Jan 1 00:00 beta\r\n")
                            .await;
                        let _ = d.shutdown().await;
                    }
                }
                send(&mut w, b"226 done\r\n").await;
            }
            "QUIT" => {
                send(&mut w, b"221 bye\r\n").await;
                break;
            }
            _ => send(&mut w, b"200 ok\r\n").await,
        }
    }
}

async fn send(w: &mut tokio::net::tcp::OwnedWriteHalf, bytes: &[u8]) {
    let _ = w.write_all(bytes).await;
}

#[tokio::test]
async fn ftp_list_dir_roundtrip() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((ctrl, _)) = listener.accept().await {
            handle(ctrl).await;
        }
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    let mut fs = connect_ftp("127.0.0.1", addr.port(), "u", "p")
        .await
        .expect("ftp connect");
    let entries = fs.list_dir("/").await.expect("list_dir");
    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();

    assert!(names.contains(&"alpha.txt".to_string()), "got {names:?}");
    assert!(
        entries.iter().any(|e| e.name == "beta" && e.kind == FileKind::Dir),
        "beta dir missing: {names:?}"
    );
}
