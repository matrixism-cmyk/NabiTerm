//! **직렬 콘솔**(COM 포트) — 장비에 콘솔 케이블로 붙는 길.
//!
//! ## 왜 필요한가
//!
//! 2026-09-09에 경쟁 제품의 기능 목록과 견줘 봤더니, MobaXterm·PuTTY·WindTerm 이 모두
//! 갖춘 것 가운데 우리에게 **통째로 없는 것**이 이것이었다. 스위치·라우터·산업 장비는
//! 아직도 콘솔 포트로 처음 설정을 받고, 네트워크가 죽으면 그 길밖에 없다.
//!
//! 폐쇄망을 쓰는 자리에서는 더하다. SSH 로 못 들어가는 상태를 푸는 것이 콘솔이다.
//!
//! ## pane 에 어떻게 붙는가
//!
//! pane 의 입력 싱크는 [`nabi_pty::ByteChannel`] 하나뿐이다(쓰기 + 크기 변경). 직렬은
//! 그 규격에 그대로 들어간다 — 쓰기는 포트로, 크기 변경은 **아무것도 하지 않는다**.
//! 직렬 선 너머에는 창이 없기 때문이다(SIGWINCH 를 받을 상대가 없다).
//!
//! 출력은 PTY 와 같은 길로 흐른다. 리더 스레드가 읽어 출력 버스에 넣으면 그다음은
//! 터미널 모델이 똑같이 처리한다 — 직렬이라고 다르게 그리지 않는다.
//!
//! ## 무엇을 하지 않는가
//!
//! 흐름 제어(RTS/CTS·XON/XOFF)는 넣지 않았다. 장비 콘솔은 거의 쓰지 않고, 잘못 켜면
//! **아무 글자도 안 나오는데 이유를 알 수 없는** 상태가 된다. 필요해지면 설정으로 낸다.

mod cfg;

pub use cfg::{sort_ports, PortInfo, SerialCfg, BAUDS, DEFAULT_BAUD};

use bytes::Bytes;
use crossbeam_channel::Sender;
use nabi_pty::ByteChannel;
use nabi_types::{GridSize, PaneId};
use std::io;
use std::time::Duration;

/// 한 번 읽기 크기. 직렬은 초당 수십 KB 를 넘지 않으므로 작게 잡는다.
const READ_BUF: usize = 4096;

/// 읽기가 이 시간 안에 아무것도 못 받으면 한 번 돌아온다(스레드가 영영 잠들지 않게).
const READ_TIMEOUT: Duration = Duration::from_millis(200);

/// 이 기계에 보이는 직렬 포트들(사람이 세는 차례로).
///
/// 못 읽으면 **빈 목록**이다. 열거가 실패하는 것과 포트가 없는 것을 화면에서 나눌 이유가
/// 없다 — 어느 쪽이든 사용자가 할 일은 같다(케이블·드라이버 확인).
pub fn ports() -> Vec<PortInfo> {
    let mut out: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| PortInfo { name: p.port_name, detail: describe(&p.port_type) })
        .collect();
    sort_ports(&mut out);
    out
}

/// 무엇이 꽂혀 있는지 한 줄로. 모르면 빈 글(빈 칸을 그리지 않는다).
fn describe(t: &serialport::SerialPortType) -> String {
    match t {
        serialport::SerialPortType::UsbPort(u) => {
            // 제품 이름이 있으면 그것이 가장 알아보기 쉽다(`USB-SERIAL CH340` 같은).
            u.product.clone().or_else(|| u.manufacturer.clone()).unwrap_or_default()
        }
        serialport::SerialPortType::BluetoothPort => "Bluetooth".into(),
        serialport::SerialPortType::PciPort => "PCI".into(),
        serialport::SerialPortType::Unknown => String::new(),
    }
}

/// 열린 포트 — pane 의 입력 싱크.
pub struct SerialChannel {
    port: Box<dyn serialport::SerialPort>,
}

impl ByteChannel for SerialChannel {
    fn write(&mut self, data: &[u8]) -> io::Result<()> {
        use std::io::Write;
        self.port.write_all(data)?;
        self.port.flush()
    }

    /// 직렬 선 너머에는 창이 없다 — 알릴 상대가 없으므로 아무것도 하지 않는다.
    fn resize(&mut self, _size: GridSize) -> io::Result<()> {
        Ok(())
    }
}

/// 포트를 열고 리더 스레드를 띄운다.
///
/// 실패는 **코드로** 나른다(`Coded`) — 아래 크레이트는 화면 언어를 모르고, 알 이유도
/// 없다. 가장 흔한 실패는 "다른 프로그램이 이미 쓰고 있다"인데, 원문만 던지면
/// `Access is denied.` 한 줄이라 무엇을 해야 할지 알 수 없다.
pub fn open(
    pane: PaneId,
    port_name: &str,
    cfg: SerialCfg,
    out_tx: Sender<(PaneId, Bytes)>,
    on_closed: Box<dyn FnOnce() + Send>,
) -> io::Result<SerialChannel> {
    let port = serialport::new(port_name, cfg.baud)
        .data_bits(data_bits(cfg.data_bits))
        .parity(parity(cfg.parity))
        .stop_bits(stop_bits(cfg.stop_bits))
        .timeout(READ_TIMEOUT)
        .open()
        .map_err(|e| explain(port_name, e))?;
    // 읽기는 복제한 손잡이로 한다 — 쓰기와 같은 것을 나눠 쓰면 서로를 막는다.
    let reader = port.try_clone().map_err(|e| explain(port_name, e))?;
    spawn_reader(pane, reader, out_tx, on_closed);
    Ok(SerialChannel { port })
}

/// 리더 스레드 — 읽은 것을 출력 버스로 흘린다(PTY 와 같은 길).
fn spawn_reader(
    pane: PaneId,
    mut port: Box<dyn serialport::SerialPort>,
    out_tx: Sender<(PaneId, Bytes)>,
    on_closed: Box<dyn FnOnce() + Send>,
) {
    let _ = std::thread::Builder::new()
        .name(format!("serial-reader-{}", pane.get()))
        .spawn(move || {
            use std::io::Read;
            let mut buf = [0u8; READ_BUF];
            loop {
                match port.read(&mut buf) {
                    Ok(0) => continue, // 잠깐 조용한 것뿐이다 — 직렬에는 EOF 가 없다.
                    Ok(n) => {
                        if out_tx.send((pane, Bytes::copy_from_slice(&buf[..n]))).is_err() {
                            break; // pane 이 닫혔다.
                        }
                    }
                    // 시간이 다 된 것은 오류가 아니다. 그 밖의 오류는 선이 빠진 것이다.
                    Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                    Err(_) => {
                        // **조용히 죽지 않는다.** 직렬은 끊겨도 화면이 그대로 남아 있어,
                        // 알리지 않으면 "장비가 아무 말이 없다"와 구분되지 않는다.
                        on_closed();
                        break;
                    }
                }
            }
        });
}

/// 여는 데 실패한 까닭을 코드로 옮긴다.
fn explain(port: &str, e: serialport::Error) -> io::Error {
    let code = match e.kind() {
        // 다른 프로그램이 쥐고 있다 — 직렬에서 가장 흔한 실패다.
        serialport::ErrorKind::Io(io::ErrorKind::PermissionDenied) => "serial.busy",
        serialport::ErrorKind::NoDevice => "serial.nodevice",
        _ => "serial.openfail",
    };
    // 원문도 함께 나른다 — 무엇이 실제로 났는지 잃으면 진단할 수 없다.
    io::Error::other(nabi_error::Coded::with(code, [port.to_string(), e.to_string()]))
}

fn data_bits(n: u8) -> serialport::DataBits {
    match n {
        5 => serialport::DataBits::Five,
        6 => serialport::DataBits::Six,
        7 => serialport::DataBits::Seven,
        _ => serialport::DataBits::Eight,
    }
}

fn parity(c: char) -> serialport::Parity {
    match c {
        'E' => serialport::Parity::Even,
        'O' => serialport::Parity::Odd,
        _ => serialport::Parity::None,
    }
}

fn stop_bits(n: u8) -> serialport::StopBits {
    match n {
        2 => serialport::StopBits::Two,
        _ => serialport::StopBits::One,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 열거는 **터지지 않아야 한다** — 포트가 없는 기계에서도 빈 목록이다.
    #[test]
    fn 포트_열거는_터지지_않는다() {
        let v = ports();
        // 이름은 비어 있으면 안 된다(빈 이름은 열 수도 고를 수도 없다).
        assert!(v.iter().all(|p| !p.name.is_empty()));
    }

    /// 없는 포트를 열면 **까닭이 담긴** 오류가 온다(원문만 던지지 않는다).
    #[test]
    fn 없는_포트는_까닭을_말한다() {
        let (tx, _rx) = crossbeam_channel::unbounded();
        let Err(e) = open(PaneId::new(1), "COM_NOPE_999", SerialCfg::default(), tx, Box::new(|| {})) else {
            panic!("없는 포트가 열렸다");
        };
        let coded = e.get_ref().and_then(|r| r.downcast_ref::<nabi_error::Coded>());
        let coded = coded.expect("코드가 실려 있어야 한다");
        assert!(coded.code.starts_with("serial."), "{}", coded.code);
        // 어느 포트였는지 함께 담아야 한다 — 여러 개를 시도할 때 구분이 된다.
        assert!(coded.args.iter().any(|a| a.contains("COM_NOPE_999")));
    }

    /// **실물 포트로 여는 길**까지 확인한다(하드웨어가 있어야 하므로 기본은 건너뜀).
    ///
    /// 순수 함수 시험은 규칙만 증명한다 — 실제로 열리는지, 쓴 것이 나가는지는 별개다.
    /// 이 저장소에서 SFTP 도 두 번 다 **실서버에서만** 결함이 나왔다.
    ///
    /// ```text
    /// cargo test -p nabi-serial -- --ignored --nocapture
    /// ```
    /// 환경변수 `NABI_SERIAL_PORT` 로 포트를 고를 수 있다(기본 COM1).
    #[test]
    #[ignore = "직렬 포트가 있는 기계에서만"]
    fn 실물_포트를_열고_쓴다() {
        let name = std::env::var("NABI_SERIAL_PORT").unwrap_or_else(|_| "COM1".into());
        let (tx, _rx) = crossbeam_channel::unbounded();
        let mut ch = match open(PaneId::new(1), &name, SerialCfg::default(), tx, Box::new(|| {})) {
            Ok(c) => c,
            Err(e) => panic!("{name} 을 열지 못했다: {e}"),
        };
        // 쓰기가 오류 없이 나가야 한다. 선 너머에 장비가 없으면 답은 안 오지만,
        // **나가는 길**은 여기서 증명된다(포트가 열렸고 드라이버가 받았다).
        ch.write(b"\r\n").expect("직렬로 쓰지 못했다");
        // 크기 변경은 아무것도 하지 않지만 오류를 내서도 안 된다(라우터가 매번 부른다).
        ch.resize(GridSize::new(80, 24)).expect("resize 가 오류를 냈다");
        println!("{name} 열기·쓰기 확인");
    }

    /// 설정한 값이 그대로 넘어가야 한다(비트 수·패리티를 잘못 옮기면 글자가 깨진다).
    #[test]
    fn 설정을_그대로_옮긴다() {
        assert!(matches!(data_bits(7), serialport::DataBits::Seven));
        assert!(matches!(data_bits(8), serialport::DataBits::Eight));
        assert!(matches!(parity('E'), serialport::Parity::Even));
        assert!(matches!(parity('O'), serialport::Parity::Odd));
        assert!(matches!(parity('N'), serialport::Parity::None));
        assert!(matches!(stop_bits(2), serialport::StopBits::Two));
        assert!(matches!(stop_bits(1), serialport::StopBits::One));
    }
}
