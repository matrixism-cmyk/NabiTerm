//! SFTP 원격 파일 작업 메시지 DTO(원격 파일 브라우저용).

use serde::{Deserialize, Serialize};

/// SFTP 연결 식별자(UI가 부여, pane과 별개).
pub type SftpId = u64;

/// 원격 디렉터리 항목. nabi-fs `FileEntry`의 proto 표현(proto→fs 의존 회피).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SftpEntry {
    pub name: String,
    pub is_dir: bool,
    /// 심볼릭 링크 여부(아이콘 구분용). 링크가 가리키는 대상 종류와 무관.
    #[serde(default)]
    pub is_link: bool,
    pub size: u64,
    /// POSIX 권한 비트(0=알 수 없음).
    pub mode: u32,
    /// 수정 시각(unix 초, 0=알 수 없음).
    pub mtime: u64,
}
