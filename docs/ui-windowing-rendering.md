# UI · 윈도잉 · GPU 렌더링

← [개발 계획서 마스터](./DEVELOPMENT_PLAN.md) · 관련 [architecture.md](./architecture.md)

이 문서는 MobaXterm식 "한 윈도우 내 다중 탭/분할 + 탭을 별도 OS 창으로 분리" UX와 GPU 터미널 렌더러의 설계를 다룹니다.

---

## 1. 멀티 OS 윈도우 (멀티 뷰포트)

✅ egui는 **멀티 뷰포트**(여러 네이티브 OS 창)를 0.24부터 지원(`ViewportId`/`ViewportBuilder`, `Context::show_viewport_deferred`/`show_viewport_immediate`). eframe 네이티브 백엔드에서 동작(웹은 미지원).

**선택: deferred viewport.** 각 OS 창은 `show_viewport_deferred`로 띄우고 **독립적으로 repaint**한다. 한 창의 바쁜 터미널이 다른 창을 강제 repaint하지 않음(immediate viewport는 부모와 lockstep → N×CPU 낭비라 회피).

- `nabi-ui-window::window_manager.rs`가 N개 viewport를 소유. 각 viewport는 자신의 `DockState<PaneId>` 보유.
- ⚠️ **deferred viewport는 새 출력이 와도 명시적으로 깨우지 않으면 "멈춘 것처럼" 보임.** → 라우터가 새 PaneOutput 시 그 PaneId를 호스팅한 viewport에 `ctx.request_repaint_of(viewport_id)` 호출(`repaint_wake.rs`).
- ⚠️ deferred viewport 간 통신은 반드시 **채널 또는 Arc/Mutex**(egui 규약). 공유 가변 상태 금지 — 우리 설계는 오케스트레이터 채널로 통일.

## 2. 탭 분리(tear-out)을 별도 OS 창으로

⚠️ **검증 정정(중요):** `egui_dock`의 tear-off는 결과가 **인-앱 플로팅 `egui::Window`**(부모 창 내부)이지 **OS 네이티브 top-level 창이 아니다.** `egui_tiles`는 tear-off 자체가 없다(단일 도크 영역 내 재배치만). → **MobaXterm식 네이티브 창 분리는 어느 크레이트도 기본 제공하지 않으므로 직접 만든다.**

**자작 tear-out 알고리즘(`tear_off.rs`):**
1. 탭 드래그가 창 밖으로 나가는 제스처 감지(또는 메뉴/단축키 `Ctrl+Shift+O` "Tear Tab into New Window").
2. 소스 `DockState<PaneId>`에서 해당 `PaneId` 제거.
3. `show_viewport_deferred`로 새 deferred viewport 생성, 그 PaneId 하나를 담은 새 `DockState`로 시드.
4. **오케스트레이터의 Pane는 손대지 않는다** — ConPTY/SSH 세션과 펌프는 그대로. 옮긴 것은 `PaneId`뿐.
5. 역방향(창→창 병합, 탭을 기존 창으로 이동)도 동일하게 PaneId 이동(`Window` 메뉴의 Merge/Move).

> 핵심: **UI는 PaneId만 보유**(원칙 #3). 그래서 tear-out/merge/close가 전부 PaneId 이동이고 desync/누수/패닉이 없다. 리뷰에서 "터미널 상태가 DockState나 viewport 클로저로 새지 않는지"를 강제.

## 3. 인-윈도우 도킹·분할·탭 (egui_dock 0.19.1)

- ✅ egui_dock는 탭 열기/닫기, 노드 간 탭 이동, 리사이즈, 중첩 분할(수평/수직)을 제공 → MobaXterm식 MDI에 충분.
- `nabi-ui-tab::tab_viewer.rs`가 `TabViewer<Tab = PaneId>` 구현: PaneId로 오케스트레이터에서 pane 조회 → 그 grid에 대한 `nabi-render` paint 콜백 발행. 탭은 PaneId만 들고, 모든 읽기는 오케스트레이터 핸들(채널/Arc 스냅샷) 경유.
- 분할 단축키: `Ctrl+Shift+\`(우측 수직), `Ctrl+Shift+-`(아래 수평), 패널 포커스 `Alt+화살표`, 최대화 `Ctrl+Shift+Z`.

## 4. 메뉴바 & 단축키 & 커맨드 팔레트

- 메뉴바는 `TopBottomPanel::top` 안에 `egui::containers::menu::MenuBar`. 최상위 메뉴마다 자기 모듈(`menu_file.rs` 등)로 분리해 라인 캡 준수.
- ⚠️ egui의 기본 Alt+F/키보드 서브메뉴 네비게이션은 약함 → **전역 가속기 테이블을 자작**(`shortcuts.rs`): `KeyboardShortcut` + `consume_shortcut`로 직접 처리.
- **메뉴 아이템은 상태를 직접 변경하지 않고 `nabi-proto::Command`만 방출** → 메뉴/팔레트/패널이 동일 경로. 커맨드 팔레트(`palette.rs`, `Ctrl+Shift+P`)는 같은 Command 카탈로그를 노출해 항상 패리티.
- 전체 메뉴 트리·단축키는 [menus-features.md](./menus-features.md).

## 5. GPU 터미널 렌더러 (`nabi-render`)

✅ egui 앱 안에 커스텀 wgpu 렌더러를 `egui_wgpu::CallbackTrait`로 임베드(prepare/paint 단계에서 공유 `Device`/`Queue`로 자체 렌더 패스). 레퍼런스: egui `custom3d_wgpu` 데모, pop-os/cosmic-term(alacritty_terminal + cosmic-text + glyphon).

**파이프라인:**
1. `instance_build.rs`: 가시 그리드를 인스턴스 배열로(셀당 bg quad + glyph quad). instancing 덕에 빈 셀 거의 무료.
2. `shaper.rs`: rustybuzz로 셰이핑 + swash로 래스터화. `run_cache.rs`(shaped-run 캐시)/`glyph_cache.rs`로 무거운 출력 시 CPU 비용 흡수.
3. 아틀라스: **grayscale 커버리지 아틀라스(R8)** + 컬러(이모지/이미지)용 RGBA 아틀라스. `etagere`로 패킹(`atlas_alloc.rs`/`atlas_upload.rs`).
4. 패스: `pass_bg.rs`(배경 instanced quads) → `pass_text.rs`(텍스트) → `pass_cursor.rs`(커서/선택 quad) → `pass_image.rs`(인라인 이미지). egui 렌더 패스 내부에서 clip rect 적용.
5. **재그리기 게이트:** `nabi-vt`의 damage 상태로만 dirty 프레임 발생.

**안티앨리어싱/감마(✅ 검증):** egui 합성 환경(투명·둥근 모서리 레이어 위 텍스트)에서는 **subpixel/ClearType가 깔끔히 합성되지 않음**(채널별 커버리지 → dual-source/2패스 필요, 투명 표면 통과 불가). 현대 터미널(Kitty linear, Ghostty grayscale R8)을 따라 **grayscale 커버리지 + linear(감마 정확) 공간 블렌드 + sRGB 인코딩** 채택. ⚠️ linear 블렌드는 텍스트가 얇아 보이므로 **contrast/gamma 보정 커브 추가**(Kitty가 하는 방식). 기본은 native 느낌과 맞추되 linear 토글 제공.

## 6. 인라인 이미지 프로토콜 (M3)

- `image_kitty.rs`(우선), `image_sixel.rs`(폴백), `image_iterm.rs`. 디코드(rasteroid/icy_sixel 등) → 텍스처 quad를 `pass_image.rs`에서 텍스트 뒤에 그림. 상태기반 파싱이므로 독립 서브시스템으로 격리.

## 7. wgpu 버전 동기화 (⚠️ 빌드 실패 방지)

paint 콜백이 저수준 wgpu 타입(`Device`/`Queue`/`RenderPass`/`CommandEncoder`)을 경계로 넘기므로 **egui-wgpu·wgpu·(사용 시)glyphon이 동일 major wgpu여야** 함. major 불일치 = 하드 컴파일 실패(egui 과거 #4850/#4905/#5476 전례). → **egui-wgpu가 정하는 wgpu를 단일 진실원**으로 루트 `[workspace.dependencies]`에 고정. glyphon을 직접 쓰면 `(egui-wgpu, glyphon, wgpu)` trio 동시 호환 확인. cosmic-text는 셰이핑/아틀라스에만 쓰고 그리드 셰이더는 자작하는 길도 유효(cosmic-term 방식).
