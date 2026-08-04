# 다국어(i18n) · CJK 입력 · 렌더링 · 인코딩

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md) · 관련 [ui-windowing-rendering.md](./ui-windowing-rendering.md)

요구: **UI 한국어/영어/일본어 지원**, 그리고 터미널 안에서의 **CJK 입력(IME)·렌더링·인코딩**. 두 층으로 나뉜다 — (A) 앱 UI 자체의 번역(i18n), (B) 터미널 내용의 CJK 처리.

---

## 1. UI 국제화 (`nabi-i18n` 신규 크레이트)

- **선택: Project Fluent 기반.** `fluent-templates 0.14`(MIT/Apache)의 **`ArcLoader`**로 런타임 ko/en/ja 전환 + 개발 중 핫리로드. (또는 `i18n-embed 0.16` — `fluent-system` + `desktop-requester`(sys-locale)로 OS 언어 자동 감지 + `rust-embed`로 .ftl을 .exe에 임베드. 둘 중 하나만.)
- **왜 Fluent(≠ rust-i18n):** 한국어·일본어의 복수/조수사 등 문법 선택을 ICU MessageFormat식으로 제대로 처리. rust-i18n은 단순 key-value라 plural 카테고리가 약함.
- **API:** 얇은 `tr!("key")` / `tr!("key", n = 3)` 매크로 + `set_language(LanguageIdentifier)`로 활성 번들 교체. .ftl 파일은 `rust-embed`로 임베드해 단일 exe가 ko/en/ja를 모두 포함.
- ⚠️ **egui는 immediate-mode** → 매 프레임 활성 언어를 다시 읽으므로 런타임 전환이 사실상 무료(위젯 트리 재구성 불필요). 단, `set_language`가 스스로 redraw를 트리거하진 않음 → 다음 repaint에 반영(언어 변경 시 `request_repaint` 호출).
- 초기 언어: `sys-locale`(또는 i18n-embed `desktop-requester`)로 Windows UI 언어 감지, 설정에서 수동 변경. → **Settings › General/Startup**의 Language 항목.
- 모듈(라인 캡 준수): `nabi-i18n/loader.rs`, `lang.rs`(LanguageIdentifier 관리/전환), `macros.rs`(`tr!`), `locales/{en,ko,ja}.ftl`(데이터).

## 2. CJK 입력 — IME (in `nabi-ui`)

터미널은 커스텀 wgpu 위젯이라 egui `TextEdit`를 거치지 않으므로 **IME 이벤트를 직접 처리**한다.
- ✅ egui `Event::Ime(ImeEvent::{Enabled, Preedit(String), Commit(String), Disabled})` 수신(winit `Ime`에서 유래).
- **Preedit:** 조합 중 문자열을 보관하고 PTY 커서 위치에 **밑줄 친 조합 텍스트 + 조합 캐럿**으로 그린다(조합 폭은 `unicode-width`로 측정).
- **Commit:** 확정된 UTF-8 문자열을 포커스된 pane의 PTY(`ByteChannel`)에 그대로 write.
- **후보창 위치:** 매 프레임 `ctx.output_mut(|o| o.ime = Some(IMEOutput { rect, cursor_rect }))`를 설정 → egui-winit이 winit `set_ime_cursor_area`로 전달 → OS 한/일 후보창이 터미널 커서 위치에 뜸.
- **게이팅:** 터미널 pane가 포커스일 때만 `set_ime_allowed(true)`.
- ⚠️ **egui의 Windows IME는 릴리스마다 회귀 이력 있음**(#3532 v0.23 중국어 구두점, #2317 IME 전환). → **핀 고정한 egui 0.34에서 MS 한국어/일본어 IME로 Preedit/Commit이 실제로 방출되는지 출시 전 실측**. (Linux fcitx5/ibus는 한국어 동작 확인됨, Windows는 "지원되나 과거 버그 있음"으로 간주하고 검증 버퍼 확보.)
- 모듈: `nabi-ui/ime.rs`(이벤트 처리), `ime_preedit.rs`(조합 텍스트 렌더), `ime_rect.rs`(커서 rect 보고).

## 3. CJK 렌더링 (이중폭 + 폰트 폴백)

**이중폭(double-width):** ✅ `alacritty_terminal`이 East Asian Wide 셀을 `Flags::WIDE_CHAR`(글리프 셀) / `WIDE_CHAR_SPACER`(뒤 채움) / `LEADING_WIDE_CHAR_SPACER`(앞 채움, 줄끝 wrap)로 태깅(`unicode-width` 기반). → 렌더러는 `WIDE_CHAR` 셀에 **2칸 예약**, spacer 셀은 그리지 않음.

⚠️ **폰트 폴백 — 중요한 설계 결정(검증 정정):** 폰트 폴백은 **cosmic-text의 상위 `FontSystem`(Fallback 트레잇)에 있고, `swash`/`rustybuzz` 단독에는 없다.** 현재 우리 렌더러 스택은 저수준 `rustybuzz`+`swash`다. 따라서 **둘 중 하나를 택해야 한다:**

- **옵션 A — 직접 폴백 구현(저수준 유지):** `fontdb`로 시스템 폰트를 열거하고, **코드포인트별로 글리프를 가진 폰트를 찾아** 런(run)을 그 폰트로 셰이핑. Windows CJK 폴백 대상: **Malgun Gothic**(한글; Win10/11 기본 한국어 폰트), **Yu Gothic / MS Gothic**(일본어; MS Gothic의 라틴은 등폭). `nabi-render/font_fallback.rs`에서 코드포인트→폰트 매칭 체인 구성.
- **옵션 B — cosmic-text `FontSystem` 채택:** 셰이핑+폴백을 cosmic-text에 맡기고 결과 글리프를 우리 wgpu 아틀라스에 업로드. CJK 폴백을 "무료"로 얻음(cosmic-term이 입증). ⚠️ 단 현행 cosmic-text는 셰이핑에 **HarfRust**를 쓰며 우리의 rustybuzz 핀과 다름 → 스택 일원화 필요.

> **권장:** 등폭 그리드 제어를 우선한다면 **옵션 A**(fontdb 직접 폴백)로 시작하되, CJK 커버리지/조판 품질이 부족하면 **옵션 B**(cosmic-text FontSystem)로 전환할 수 있도록 셰이핑/폴백을 `nabi-render` 내 트레잇 경계 뒤에 둔다. (cosmic-term issue #325의 CJK 겹침/초기 렌더 지연 같은 알려진 거친 부분 감안.)

## 4. 문자 인코딩 (레거시 CJK)

- ✅ **`encoding_rs 0.8.35`**(Firefox 엔진) 디코딩 탭을 **`nabi-vt` 안 ByteChannel 바이트와 VT 파서 사이**에 둔다.
- **기본 UTF-8**(nabi의 선택 — encoding_rs 자체엔 기본값 없음, 명시 선택 필요). 레거시 세션은 `new_decoder()`로 상태 있는 `Decoder` 생성 후 `decode_to_utf8`로 **PTY 청크마다 스트림 디코딩**(멀티바이트가 패킷 경계에 걸쳐도 디코더가 상태를 이어받아 정확). 
- 지원: **Shift_JIS, EUC-JP, EUC-KR(=Windows-949/CP949 매핑), GBK/GB18030(동일 디코더), Big5**, ISO-2022-JP.
- 세션별 오버라이드 → **Terminal › Character Encoding** 서브메뉴. 모듈: `nabi-vt/decode_tap.rs`(스트림 디코더 래퍼), `encoding.rs`(라벨↔Encoding 매핑).

## 5. 추가/변경 크레이트 핀

| 크레이트 | 버전 | 라이선스 | 용도 |
|----------|------|----------|------|
| fluent-templates | 0.14.0 | MIT/Apache | UI i18n(ArcLoader 런타임 ko/en/ja) |
| i18n-embed | 0.16.0 | MIT | (대안) 임베드 + OS 언어 자동감지 |
| sys-locale | 최신 | MIT/Apache | 초기 언어 감지 |
| encoding_rs | 0.8.35 | (Apache/MIT) AND BSD-3 | 레거시 CJK 디코딩 |
| unicode-width | 최신 | MIT/Apache | 이중폭/조합 문자열 폭 측정 |
| fontdb | (cosmic-text 동봉/직접) | MIT/Apache | 시스템 폰트 열거(폴백 체인) |
