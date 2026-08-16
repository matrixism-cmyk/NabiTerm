//! 동적(-D) 포워딩: 로컬 SOCKS5 프록시 → SSH direct-tcpip.
//!
//! 최소 SOCKS5(no-auth, CONNECT, IPv4/도메인)만 처리한다.

use crate::forward::{connect_authed, Fwd};
use nabi_proto::SshParams;
use russh::client::Handle;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 로컬 SOCKS5 프록시를 띄우고 바인드된 포트를 반환한다. 각 연결의 목적지를
/// SSH direct-tcpip로 포워딩한다.
pub async fn start_dynamic_forward(params: SshParams) -> Result<u16, String> {
    let handle = Arc::new(connect_authed(&params).await?);
    let listener = TcpListener::bind(("127.0.0.1", 0u16))
        .await
        .map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let h = handle.clone();
            tokio::spawn(async move {
                let _ = serve(socket, h).await;
            });
        }
    });
    Ok(port)
}

async fn serve(mut socket: TcpStream, handle: Arc<Handle<Fwd>>) -> Result<(), String> {
    let (host, port) = handshake(&mut socket).await?;
    let channel = handle
        .channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| e.to_string())?;
    let mut stream = channel.into_stream();
    tokio::io::copy_bidirectional(&mut socket, &mut stream)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// SOCKS5 핸드셰이크를 처리하고 목적지 (host, port)를 반환한다.
async fn handshake(socket: &mut TcpStream) -> Result<(String, u16), String> {
    let mut greet = [0u8; 2];
    socket.read_exact(&mut greet).await.map_err(io)?;
    let mut methods = vec![0u8; greet[1] as usize];
    socket.read_exact(&mut methods).await.map_err(io)?;
    socket.write_all(&[0x05, 0x00]).await.map_err(io)?; // no-auth

    let mut req = [0u8; 4];
    socket.read_exact(&mut req).await.map_err(io)?; // ver, cmd, rsv, atyp
    let host = match req[3] {
        0x01 => {
            let mut a = [0u8; 4];
            socket.read_exact(&mut a).await.map_err(io)?;
            format!("{}.{}.{}.{}", a[0], a[1], a[2], a[3])
        }
        0x03 => {
            let mut len = [0u8; 1];
            socket.read_exact(&mut len).await.map_err(io)?;
            let mut d = vec![0u8; len[0] as usize];
            socket.read_exact(&mut d).await.map_err(io)?;
            String::from_utf8_lossy(&d).into_owned()
        }
        _ => return Err(nabi_i18n::trc("net.socks.atyp").into()),
    };
    let mut pb = [0u8; 2];
    socket.read_exact(&mut pb).await.map_err(io)?;
    let port = u16::from_be_bytes(pb);

    // 성공 응답(BND.ADDR/PORT는 0).
    socket
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(io)?;
    Ok((host, port))
}

fn io(e: std::io::Error) -> String {
    e.to_string()
}
