//! russh 클라이언트 핸들러(호스트키 검증 — known_hosts TOFU + 사용자 확인).

use crate::verify::{HostKeyInfo, HostKeyVerifier};
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
        }
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

    async fn check_server_key(&mut self, key: &PublicKey) -> Result<bool, Self::Error> {
        match russh::keys::check_known_hosts_path(&self.host, self.port, key, &self.known_hosts) {
            Ok(true) => Ok(true),
            Ok(false) => {
                // 미지 호스트: verifier가 있으면 사용자에게 확인, 없으면 자동 학습(TOFU).
                let Some(v) = &self.verifier else {
                    self.learn(key);
                    return Ok(true);
                };
                let info = HostKeyInfo {
                    host: self.host.clone(),
                    port: self.port,
                    algorithm: key.algorithm().to_string(),
                    fingerprint: crate::fingerprint::sha256_fingerprint(key),
                };
                let accept = v.verify(info).await.unwrap_or(false);
                if accept {
                    self.learn(key);
                }
                Ok(accept)
            }
            Err(_) => Ok(false),
        }
    }
}
