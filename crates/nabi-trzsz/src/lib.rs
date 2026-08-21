//! trzsz(`trz`/`tsz`) 파일 전송 프로토콜 — **클라이언트 쪽** 순수 구현.
//!
//! 왜 필요한가: SFTP가 있어도 `sz`/`rz`는 대체되지 않는다. 점프 호스트·`sudo -i`·컨테이너
//! `exec`·시리얼 콘솔처럼 **별도 채널을 못 여는 자리**에서는 이미 열려 있는 셸이 유일한 통로다.
//! lrzsz는 AL2023부터 빠지는 중이라 후계인 trzsz를 먼저 지원한다(사용자 요청 2026-08-21).
//!
//! 이 크레이트는 터미널·SSH·PTY를 모른다. **바이트 in → 바이트 out + 상태기계**뿐이라
//! 전부 순수 함수로 시험할 수 있다. 실제 배선은 nabi-orchestrator의 라우터가 한다.
//!
//! 프로토콜 사실은 trzsz-go(MIT) 원본의 와이어 포맷을 읽어 확인했다. 코드는 옮기지 않았다 —
//! 포맷은 사실이고, 구현은 우리 것이다(nabiTerm은 Apache-2.0).

mod action;
mod codec;
mod line;
mod progress;
mod session;
mod trigger;
mod upload;

pub use codec::{decode_payload, encode_payload};
pub use line::{render, Line, LineFramer};
pub use action::{Action, Config};
pub use progress::{Progress, Rate};
pub use session::{FileSink, FileSource, Plan, Session, Step, Storage, UploadItem};
pub use trigger::{Mode, Scanned, Trigger, TriggerScanner};

/// 우리가 원격에 알리는 클라이언트 이름·판(`#ACT`에 실린다).
pub const CLIENT_LANG: &str = "rust";
/// 원격이 버전 비교로 기능을 켜고 끈다. trzsz 1.1.x 계열과 호환되는 값을 쓴다.
pub const CLIENT_VERSION: &str = "1.1.8";

/// 우리가 제안하는 프로토콜 판. 원격은 `min(이 값, 원격 최대치)`로 정한다.
///
/// 1로 시작하는 것은 의도다 — v2는 zstd, v3·v4는 파이프라인이라 왕복이 사라지는 대신
/// 구현이 몇 배로 커진다. 속도가 필요한 전송은 이미 SFTP가 맡고 있고, 여기서 필요한 것은
/// **어떤 자리에서든 되는 것**이다. 측정한 뒤에 올린다.
pub const PROTOCOL_VERSION: u32 = 1;
