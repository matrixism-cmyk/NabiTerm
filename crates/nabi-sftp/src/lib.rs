//! nabi-sftp — SFTP 백엔드(russh-sftp). `nabi_fs::RemoteFs` 구현.
//!
//! ⚠️ 구조 구현(컴파일 검증). 런타임 동작 확인에는 실제 SSH 서버가 필요하다.
//! MobaXterm식으로 기존 SSH 연결을 재사용하려면 오케스트레이터에서 핸들을 공유하도록
//! 확장한다(현재는 별도 연결 + 비밀번호 인증).

pub mod fs;
pub mod hashcheck;
mod pipeline;
pub mod raw;
mod recurse;
pub mod session;
mod xfer;

pub use fs::SftpFs;
pub use hashcheck::SFTP_VERIFY_HASH;
pub use recurse::DirProgress;
pub use session::connect_sftp;

/// SFTP 파일명 인코딩 설정을 적용한다(설정 문자열 → 벤더 패치 charset).
///
/// v3 서버는 파일명을 서버 로컬 인코딩 raw 바이트로 보낸다(규격에 인코딩 없음) —
/// "auto"는 UTF-8 실패 시 CP949→Shift_JIS→GBK 순으로 무손실 감지하고, 송신 경로도
/// 같은 인코딩으로 복원한다. 변경 시에만 적용되므로 매 프레임 호출해도 안전하다.
pub fn set_name_charset(label: &str) {
    use russh_sftp::charset::{set_filename_charset_if_changed, Charset};
    let c = match label {
        "utf8" | "utf-8" => Charset::Utf8,
        "euc-kr" | "cp949" => Charset::EucKr,
        "shift_jis" | "sjis" => Charset::ShiftJis,
        "gbk" => Charset::Gbk,
        _ => Charset::Auto,
    };
    set_filename_charset_if_changed(c);
}

/// auto 모드에서 감지된 서버 파일명 인코딩 라벨(UI 배지용). 미감지면 None.
pub use russh_sftp::charset::detected_label as detected_name_charset;

#[cfg(test)]
mod charset_test;
#[cfg(test)]
mod sftp_server;
#[cfg(test)]
mod sftp_boot;
#[cfg(test)]
mod sftp_serverext;
#[cfg(test)]
mod sftp_test;
#[cfg(test)]
mod pipeline_test;
#[cfg(test)]
mod realserver_test;
#[cfg(test)]
mod realserver_pipe;
