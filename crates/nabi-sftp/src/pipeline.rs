//! 파이프라인 전송 — 요청을 여러 개 띄워 놓고 받는다.
//!
//! SFTP는 요청 하나를 보내고 응답을 기다리는 프로토콜이라, 한 번에 하나만 띄우면
//! 처리량이 `청크 ÷ RTT`에 묶인다(RTT 50ms·256KB면 5MB/s). 미결 요청을 N개 유지하면
//! 그 N배가 된다. OpenSSH sftp 클라이언트도 같은 이유로 기본 64를 쓴다.
//!
//! **파도(wave) 단위**로 처리한다: N개를 띄우고 전부 받은 뒤 다음 N개. 슬라이딩 윈도우보다
//! 1/N 만큼 손해지만, 오프셋 관리가 단순해져 구멍(hole)이 생길 여지가 없다.

use crate::raw::RawFs;

/// 동시에 띄우는 요청 수(기본값). OpenSSH sftp 클라이언트와 같다.
const DEFAULT_DEPTH: usize = 64;

/// 실제로 쓰는 파이프라인 깊이. `NABI_SFTP_DEPTH`로 덮어쓸 수 있다(1이면 파이프라이닝 없음).
/// 문제가 있는 서버를 만났을 때 재빌드 없이 좁힐 수 있고, 효과 측정에도 쓴다.
pub(crate) fn depth() -> usize {
    static D: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        std::env::var("NABI_SFTP_DEPTH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|d| *d > 0)
            .unwrap_or(DEFAULT_DEPTH)
            .min(256)
    })
}

/// 한 파도의 결과.
struct Wave {
    /// 앞에서부터 빈틈없이 확보한 바이트 수.
    advanced: usize,
    eof: bool,
    /// 요청보다 짧게 온 응답의 길이(있으면). 다음 파도의 조각 크기를 여기에 맞춘다.
    short: Option<usize>,
}

/// `at`부터 depth()개 읽기를 동시에 띄우고, **앞에서부터 이어지는 만큼만** sink에 넘긴다.
///
/// 서버는 요청보다 짧게 응답할 수 있다(short read). 그러면 그 뒤 조각들과 사이에 빈틈이
/// 생기므로 버리고 다시 요청한다 — 이어붙여 구멍을 만드는 것보다 낫다. 대신 그 길이를
/// 돌려줘 다음 파도부터는 서버가 실제로 채워 주는 크기로 요청하게 한다.
async fn read_wave(
    raw: &RawFs,
    handle: &str,
    at: u64,
    chunk: usize,
    n_req: usize,
    sink: &mut impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<Wave, String> {
    let reqs = (0..n_req).map(|i| raw.read_at(handle, at + (i * chunk) as u64, chunk));
    let results = futures_util::future::join_all(reqs).await;
    let (mut advanced, mut eof, mut short) = (0usize, false, None);
    for r in results {
        match r? {
            None => {
                eof = true;
                break;
            }
            Some(data) => {
                let n = data.len();
                sink(&data)?;
                advanced += n;
                if n < chunk {
                    short = (n > 0).then_some(n);
                    break;
                }
            }
        }
    }
    Ok(Wave { advanced, eof, short })
}

/// 원격 파일을 `at`부터 끝까지 읽어 sink로 흘려보낸다. 반환값은 읽은 총 바이트.
///
/// `tick`은 파도마다 누적 바이트로 호출된다 — 진행률 보고와 취소·속도제한을 여기서 한다.
pub(crate) async fn download_stream(
    raw: &RawFs,
    handle: &str,
    at: u64,
    size: Option<u64>,
    mut sink: impl FnMut(&[u8]) -> Result<(), String>,
    mut tick: impl FnMut(u64) -> Result<Option<std::time::Duration>, String>,
) -> Result<u64, String> {
    // limits가 알려주는 최대 읽기 길이와 서버가 **한 응답에 실제로 담아 주는** 길이는 다르다
    // (Windows OpenSSH 9.5는 255KB를 허용한다면서 100KB만 준다). 그래서 첫 조각은 혼자
    // 보내 실제 길이를 알아낸 뒤 그 크기로 파도를 돈다 — 안 그러면 파도마다 앞 조각 하나만
    // 쓸모 있어 요청의 대부분이 버려진다(실측: 깊이 64에서 처리량 1/25).
    let mut chunk = raw.read_chunk();
    let (mut pos, mut total) = (at, 0u64);
    match raw.read_at(handle, pos, chunk).await? {
        None => return Ok(0),
        Some(d) if d.is_empty() => return Ok(0),
        Some(d) => {
            chunk = chunk.min(d.len());
            pos += d.len() as u64;
            total += d.len() as u64;
            sink(&d)?;
        }
    }
    loop {
        // 남은 크기를 알면 그만큼만 요청한다(파일 끝 너머로 헛요청하지 않게).
        let n_req = match size {
            Some(sz) => ((sz.saturating_sub(pos) as usize).div_ceil(chunk)).clamp(1, depth()),
            None => depth(),
        };
        let w = read_wave(raw, handle, pos, chunk, n_req, &mut sink).await?;
        pos += w.advanced as u64;
        total += w.advanced as u64;
        if let Some(n) = w.short.filter(|n| *n < chunk) {
            chunk = n;
        }
        if let Some(delay) = tick(total)? {
            tokio::time::sleep(delay).await;
        }
        if w.eof || w.advanced == 0 {
            break; // 끝이거나 더 나아가지 못함(방어).
        }
        if size.is_some_and(|sz| pos >= sz) {
            break; // 알려진 크기를 다 받았다 — EOF 확인 왕복을 아낀다.
        }
    }
    Ok(total)
}

/// 로컬 파일을 `at`부터 원격 핸들에 쓴다. 반환값은 **연속으로 확인된** 총 길이.
///
/// 한 파도의 쓰기가 전부 성공해야 그 구간을 확정한다. 중간에 실패하면 확정된 접두 길이만
/// 반환하고, 호출자가 그 길이로 잘라 다음 이어올리기가 구멍을 만나지 않게 한다.
pub(crate) async fn upload_stream(
    raw: &RawFs,
    handle: &str,
    at: u64,
    mut fill: impl FnMut(usize) -> Result<Vec<u8>, String>,
    mut tick: impl FnMut(u64) -> Result<Option<std::time::Duration>, String>,
) -> Result<u64, (u64, String)> {
    let chunk = raw.write_chunk();
    let mut acked = at;
    loop {
        // 한 파도 분량을 미리 읽어 둔다(로컬 읽기는 동기라 순서가 보장된다).
        let mut parts: Vec<(u64, Vec<u8>)> = Vec::with_capacity(depth());
        let mut off = acked;
        for _ in 0..depth() {
            let buf = fill(chunk).map_err(|e| (acked, e))?;
            if buf.is_empty() {
                break;
            }
            let n = buf.len() as u64;
            parts.push((off, buf));
            off += n;
        }
        if parts.is_empty() {
            break;
        }
        let sent: u64 = parts.iter().map(|(_, b)| b.len() as u64).sum();
        let reqs = parts.into_iter().map(|(o, b)| raw.write_at(handle, o, b));
        for r in futures_util::future::join_all(reqs).await {
            r.map_err(|e| (acked, e))?; // 하나라도 실패하면 이 파도는 확정하지 않는다.
        }
        acked += sent;
        match tick(acked) {
            Ok(Some(delay)) => tokio::time::sleep(delay).await,
            Ok(None) => {}
            Err(e) => return Err((acked, e)),
        }
    }
    Ok(acked)
}

/// 이어올리기 시작점 — 남은 임시 파일에서 **구멍이 있을 수 있는 꼬리**만큼 되감은 위치.
///
/// 파도는 전부 성공해야 확정되므로, 어느 시점에나 `[0, 확정)`에는 구멍이 없고 구멍은
/// `[확정, 확정+한파도)` 안에만 생긴다. 파일 길이도 `확정+한파도`를 넘지 못한다.
/// 따라서 길이에서 한 파도를 빼면 반드시 확정 지점 이하로 내려가고, 거기서부터 끝까지
/// 다시 보내면 구멍이 사라진다 — 파일을 자르지 않아도 된다(자르기를 거부하는 서버가 있다).
pub(crate) fn resume_offset(part_len: u64, chunk: usize) -> u64 {
    part_len.saturating_sub((depth() * chunk) as u64)
}

#[cfg(test)]
mod tests {
    use super::{depth, resume_offset};

    #[test]
    fn resume_rewinds_one_wave() {
        let chunk = 32 * 1024;
        let wave = (depth() * chunk) as u64;
        assert_eq!(resume_offset(0, chunk), 0);
        assert_eq!(resume_offset(wave / 2, chunk), 0, "한 파도보다 짧으면 처음부터");
        assert_eq!(resume_offset(wave * 3, chunk), wave * 2, "마지막 파도는 다시 보낸다");
    }
}
