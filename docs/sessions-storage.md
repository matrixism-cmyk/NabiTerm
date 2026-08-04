# 세션 저장 · 내보내기/가져오기 · 저장 위치 지정(포터블)

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md) · 관련 [ssh-security.md](./ssh-security.md)

요구: **저장된 세션 목록 내보내기(export)**, **세션 저장 위치를 사용자가 지정**(포터블/USB·동기화 폴더 포함).

---

## 1. 경로 해석 (`nabi-config::paths` — 단일 진실원)

- ✅ **`directories 6.0.0`**(MIT/Apache) `ProjectDirs::from("com","aeo","nabi")`로 기본 per-user 경로: config → `%APPDATA%\aeo\nabi\config`, data(비로밍) → `%LOCALAPPDATA%\aeo\nabi\data`. ⚠️ 저수준 `dirs`는 앱 한정자 없이 루트만 주므로 비권장. ⚠️ 이들은 **Known Folder API**(`FOLDERID_RoamingAppData`/`LocalAppData`)를 호출(환경변수 직접 읽지 않음) — 로밍/리다이렉트 프로필에 더 견고.
- 모든 경로 결정을 한 곳에 모은다: `paths.rs`가 `StorageLayout { base, config_file, sessions_file, known_hosts, vault, themes }` 반환.

## 2. 저장 위치 지정 — 우선순위 체인 (WezTerm 모델)

`base_dir.rs`가 다음 순서로 base 디렉터리를 결정(figment **레이어링보다 먼저** — 닭-달걀 루프 방지):
1. `--storage-dir <경로>` (CLI 플래그)
2. `NABI_CONFIG_DIR` (환경변수)
3. **포터블 모드** — 실행파일(`std::env::current_exe()`) 옆의 마커 파일(`portable.toml`) 존재 시 모든 것을 exe 옆에 저장(USB 설치). `\\?\` verbatim 접두 제거, 쓰기 가능 검증, 불가 시 경고 후 per-user 폴백.
4. `ProjectDirs` 기본값.

반환값에 `StorageMode { Portable / PerUser / Custom }` 포함. 모듈: `base_dir.rs`, `portable.rs`, `drive_kind.rs`(`GetDriveType` → removable/fixed/network + OneDrive/Dropbox/Drive 휴리스틱 감지). UI: **Settings › Security & Vault / Storage**의 `settings/storage.rs`(위치 선택·포터블 토글·StorageMode/드라이브 경고 표시·재배치 트리거).

## 3. 세션 모델 & 저장 (`nabi-session` 신규)

- `model.rs`: `SessionEntry / SessionFolder / SessionTree`. ⚠️ **자격증명은 `vault_key`/`credential_ref`(불투명 핸들)로만 참조 — 비밀을 절대 인라인하지 않음.** 순수 데이터, I/O 없음.
- `store.rs`: 저장 세션 트리를 `StorageLayout`의 `sessions.toml`로 load/save, `nabi-config::persist`(원자적 쓰기)로. 세션 매니저 트리(`nabi-ui-panels::session_tree`)가 여기서 읽음.

## 4. 내보내기 / 가져오기

- **포맷:** 버전드·비밀 없는 휴대 포맷. `SessionExport { schema_version, exported_at, sessions: Vec<SessionEntry> }`를 **TOML(기본, 사람 친화)** + **JSON(기계 상호운용)**으로. `schema_version.rs`가 match 기반 `migrate(old)→current`(또는 `version-migrate 0.20`) 처리.
- ⚠️ **비밀 절대 직렬화 금지** — `export.rs`는 직렬화 바이트에 자격증명 바이트가 없음을 **테스트로 단언**. 가져오기 시 `vault_key` 재해석, 없으면 사용자 프롬프트.
- **상호운용(`interop_*.rs`):**
  - `interop_openssh.rs`: **OpenSSH `~/.ssh/config`로 내보내기**(Host/HostName/User/Port/IdentityFile/ProxyJump — SSH 부분집합 무손실, IdentityFile은 경로 참조라 비밀 없음) + russh-config로 가져오기.
  - `interop_putty.rs`: PuTTY `.reg`/레지스트리(`HKCU\Software\SimonTatham\PuTTY\Sessions`) 가져오기.
  - `interop_mobaxterm.rs`: MobaXterm `.mxtsessions`(CP1252 INI, `#type#icon%host%port%user%` 문법; `rust-ini 0.21.3` + encoding_rs) 가져오기/내보내기. ⚠️ **평문 비밀번호가 있으면 경고하고 볼트로 라우팅**(세션 파일에 복사하지 않음).

## 5. 보안 — 포터블/동기화 저장 (⚠️ 핵심 규칙)

- ⚠️ **DPAPI / Windows Credential Manager는 user+machine 바인딩** → 다른 PC에서 복호 불가. **포터블/동기화(USB·OneDrive 등) 저장 시에는 OS 바인딩 비밀을 절대 쓰지 않고**, 앱 자체 **master password Argon2id+AES-GCM 볼트만** 사용([ssh-security.md §8](./ssh-security.md)). `nabi-secret::vault_location.rs`가 `StorageLayout`의 볼트 경로를 받아 이 규칙을 강제.
- `drive_kind`가 removable/네트워크/클라우드 동기 폴더를 감지하면 **비차단 경고**: "포터블 저장은 master password가 필요하며 OS 바인딩 비밀은 제외됩니다."
- ⚠️ **재배치는 위험 작업** → 기존 base(config+sessions+known_hosts+vault)를 새 위치로 옮길 때 **copy-verify-then-switch 트랜잭션**(절대 in-place move 아님; 검증 전까지 기존 base 유지). 모든 쓰기는 원자적(`tempfile` 동일 디렉터리 + `sync_all` + persist/rename — 싸구려 USB 정전 대비 fsync).
- ⚠️ **클라우드 동기화되는 비암호화 sessions/known_hosts/config는 호스트 인벤토리·TOFU 키를 클라우드에 노출** → 볼트만 암호화됨을 문서화하고 경고. known_hosts·볼트 경로는 `StorageLayout`에 포함돼 base와 함께 원자적으로 이동.

## 6. 추가 크레이트 핀

| 크레이트 | 버전 | 라이선스 | 용도 |
|----------|------|----------|------|
| directories | 6.0.0 | MIT/Apache | 기본 config/data 경로(Known Folder) |
| rust-ini | 0.21.3 | MIT | PuTTY/MobaXterm INI 파싱 |
| version-migrate | 0.20.0 | (확인) | 내보내기 스키마 버전 마이그레이션(선택) |
| tempfile | 3.x | MIT/Apache | 원자적 쓰기(persist/rename) |
| blake3 | 최신 | CC0/Apache | (선택) 무결성/변경 감지 |
