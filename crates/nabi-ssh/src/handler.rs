//! russh 클라이언트 핸들러(호스트키 검증 — known_hosts TOFU + 사용자 확인).

use crate::kexinfo::{KexInfo, KexSlot};
use crate::verify::{ChangedKey, HostKeyInfo, HostKeyVerifier};
use russh::client;
use russh::keys::PublicKey;
use std::path::PathBuf;

/// 클라이언트 이벤트 핸들러.
///
/// known_hosts 정책: 알려진 키 일치 → 수락, 미지 호스트 → 사용자 확인(verifier가
/// 있으면 모달로 묻고, 없으면 자동 학습 TOFU), **키 변경/검증 오류 → 거부(MITM 보호)**.
pub struct ClientHandler {
    host: String,
    port: u16,
    known_hosts: PathBuf,
    verifier: Option<HostKeyVerifier>,
    /// 협상된 KEX·암호를 받아 갈 슬롯(T1-2 PQ 배지). None이면 기록 안 함.
    kex_slot: Option<KexSlot>,
    /// 이 세션이 에이전트 포워딩을 요청했는가. 거짓이면 서버가 열려는 에이전트 채널을 막는다.
    agent_forward: bool,
}

impl ClientHandler {
    pub fn new(
        host: String,
        port: u16,
        known_hosts: PathBuf,
        verifier: Option<HostKeyVerifier>,
    ) -> Self {
        Self {
            host,
            port,
            known_hosts,
            verifier,
            kex_slot: None,
            agent_forward: false,
        }
    }

    /// 에이전트 포워딩을 켠다(세션 설정에서 켠 경우에만).
    pub fn with_agent_forward(mut self, on: bool) -> Self {
        self.agent_forward = on;
        self
    }

    /// 협상 결과를 기록할 슬롯을 단다(연결 수립 측이 만들어 넘긴다).
    pub fn with_kex_slot(mut self, slot: KexSlot) -> Self {
        self.kex_slot = Some(slot);
        self
    }

    /// 사용자에게 묻고, 받아들이면 배운다. 미지 호스트와 바뀐 키가 **같은 길**을 쓴다 —
    /// 확인창을 두 벌 만들면 한쪽에만 안전장치가 붙는 일이 생긴다.
    async fn ask(
        &self,
        v: &crate::verify::HostKeyVerifier,
        key: &PublicKey,
        changed: Option<ChangedKey>,
    ) -> Result<bool, russh::Error> {
        let replace_line = changed.as_ref().map(|c| c.line);
        let info = HostKeyInfo {
            host: self.host.clone(),
            port: self.port,
            algorithm: key.algorithm().to_string(),
            fingerprint: crate::fingerprint::sha256_fingerprint(key),
            changed,
        };
        let accept = v.verify(info).await.unwrap_or(false);
        if accept {
            // 바뀐 키를 받아들였으면 **옛 줄을 먼저 지운다**. 안 지우면 다음 접속에도
            // 같은 경고가 뜬다(파일에 두 지문이 남아 늘 어긋난다).
            if let Some(line) = replace_line {
                if let Ok(c) = std::fs::read_to_string(&self.known_hosts) {
                    let body = crate::keychange::remove_line(&c, line);
                    if let Err(e) = std::fs::write(&self.known_hosts, body) {
                        // 못 지우면 **다음 접속에도 같은 경고가 뜬다.** 사용자는 이미
                        // 받아들였는데 또 물어보는 셈이라, 왜 그런지 알 길이 있어야 한다.
                        tracing::warn!(target: "ssh", path = %self.known_hosts.display(), %e,
                            "known_hosts 에서 옛 줄을 지우지 못했다 — 다음 접속에도 경고가 뜬다");
                    }
                }
            }
            self.learn(key);
        }
        Ok(accept)
    }

    fn learn(&self, key: &PublicKey) {
        let _ = russh::keys::known_hosts::learn_known_hosts_path(
            &self.host,
            self.port,
            key,
            &self.known_hosts,
        );
    }
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    /// 서버가 서명을 부탁하려고 여는 에이전트 채널 — 로컬 에이전트에 이어 준다.
    ///
    /// russh의 기본 구현은 채널을 **수락만 하고 아무것도 하지 않는다.** 그러면 원격이
    /// 답을 기다리며 멈춘다. 실제 통로는 우리가 놓아야 한다([`crate::agentfwd`]).
    ///
    /// 포워딩을 켜지 않은 세션이면 거절한다 — 요청하지도 않은 서버가 우리 에이전트에
    /// 손대게 두지 않는다.
    async fn server_channel_open_agent_forward(
        &mut self,
        channel: russh::Channel<client::Msg>,
        reply: client::ChannelOpenHandle,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        if !self.agent_forward {
            reply.reject(russh::ChannelOpenFailure::AdministrativelyProhibited).await;
            return Ok(());
        }
        reply.accept().await;
        tokio::spawn(async move {
            let stream = channel.into_stream();
            let (rx, tx) = tokio::io::split(stream);
            // 실패해도 세션은 계속 간다 — 원격의 그 한 번의 서명만 실패한다.
            let _ = crate::agentfwd::serve_channel(rx, tx).await;
        });
        Ok(())
    }

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        match russh::keys::check_known_hosts_path(&self.host, self.port, key, &self.known_hosts) {
            Ok(true) => Ok(true),
            Ok(false) => {
                // 미지 호스트: verifier가 있으면 사용자에게 확인, 없으면 자동 학습(TOFU).
                let Some(v) = &self.verifier else {
                    self.learn(key);
                    return Ok(true);
                };
                self.ask(v, key, None).await
            }
            // **알던 키와 다르다.** 지금까지 이 갈래는 조용히 거부만 했다 — 사용자에게는
            // 이유 없이 안 붙는 서버로 보였고, 고칠 길도 없었다.
            //
            // 중간자 공격과 서버 재설치는 겉으로 똑같이 생겼으므로 우리가 판단하지 않는다.
            // 사실(어느 줄, 옛 지문, 새 지문)만 올려 보내고 사람이 정하게 한다.
            Err(russh::keys::Error::KeyChanged { line }) => {
                let Some(v) = &self.verifier else {
                    return Ok(false); // 물어볼 곳이 없으면 거부가 안전한 쪽이다.
                };
                let old = std::fs::read_to_string(&self.known_hosts)
                    .map(|c| crate::keychange::old_fingerprint(&c, line))
                    .unwrap_or_default();
                let changed = Some(ChangedKey { line, old_fingerprint: old });
                let v = v.clone();
                self.ask(&v, key, changed).await
            }
            Err(_) => Ok(false),
        }
    }

    async fn kex_done(
        &mut self,
        _shared_secret: Option<&[u8]>,
        names: &russh::Names,
        _session: &mut client::Session,
    ) -> Result<(), Self::Error> {
        // rekey 때도 불린다 — 항상 최신 협상 결과로 덮는다.
        if let Some(slot) = &self.kex_slot {
            if let Ok(mut s) = slot.lock() {
                *s = Some(KexInfo {
                    kex: names.kex.as_ref().to_string(),
                    cipher: names.cipher.as_ref().to_string(),
                });
            }
        }
        Ok(())
    }
}
