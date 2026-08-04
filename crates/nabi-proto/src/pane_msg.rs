//! pane 단위 부가 메시지 타입.

/// OSC 133/sentinel로 검출한 한 명령의 경계 정보.
/// Serialize: 제어 평면 `wait --until command-done`이 JSON으로 회신(CP-5 G3).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CommandBlock {
    /// 실행된 명령 줄(검출되면).
    pub command_line: Option<String>,
    /// 종료 코드(OSC 133;D 또는 sentinel에서).
    pub exit_code: Option<i32>,
    /// 출력 시작 행(스크롤백 절대 행 인덱스).
    pub output_start_row: Option<usize>,
    /// 출력 끝 행.
    pub output_end_row: Option<usize>,
}
