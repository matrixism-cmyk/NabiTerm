//! 워크스페이스 전역 tracing 구독자 초기화(콘솔 + 회전 파일 로그).

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// tracing 구독자를 초기화한다(콘솔 fmt + 선택적 일별 회전 파일 + EnvFilter).
///
/// 필터는 `NABI_LOG` 환경변수에서 읽고, 없으면 `info`. 중복 초기화는 무시한다.
/// `log_dir`를 주면 `nabi.log.YYYY-MM-DD`로 비동기 기록한다 — 반환된 가드를
/// main이 보관해야 종료 시까지 플러시된다(드롭 시 로그 유실).
pub fn init(log_dir: Option<&std::path::Path>) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = std::env::var("NABI_LOG")
        .ok()
        .and_then(|s| EnvFilter::try_new(s).ok())
        .unwrap_or_else(|| EnvFilter::new("info"));

    let (file_layer, guard) = match log_dir {
        Some(dir) => {
            let _ = std::fs::create_dir_all(dir);
            let appender = tracing_appender::rolling::daily(dir, "nabi.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = fmt::layer().with_writer(writer).with_ansi(false).with_target(true);
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .with(file_layer)
        .try_init();
    guard
}
