# nabiTerm에 기여하기 / Contributing to nabiTerm

nabiTerm은 Apache License 2.0으로 공개된 프로젝트입니다. 이슈 제보와 PR을 환영합니다.

## 기여 규칙

1. **라이선스**: 모든 기여는 [Apache-2.0](LICENSE)으로 제출된 것으로 간주합니다
   (Apache-2.0 §5). 별도 CLA는 없습니다.
2. **DCO(Developer Certificate of Origin)**: 커밋에 `Signed-off-by`를 넣어 주세요
   (`git commit -s`). 본인이 기여할 권리가 있는 코드임을 확인하는 서명입니다.
   자세한 내용: <https://developercertificate.org>
3. **코드 규율**(자세한 것은 PR 리뷰에서 안내합니다):
   - 소스 파일은 소프트 250 / 하드 400 **코드 줄** 한도(주석·빈 줄 제외).
     `cargo run -p xtask -- lines` 경고 0을 유지해 주세요.
   - `cargo clippy --workspace --all-targets` 경고 0, `cargo test --workspace` 통과.
   - 새 i18n 문자열은 4-튜플(key, en, ko, ja)로 추가합니다.
   - 순수 함수 + 단위 테스트를 선호합니다. 동작이 노출되는 모든 표면
     (메뉴·팔레트·설정·i18n)을 함께 갱신해 주세요.
4. **빌드(Windows)**: MSVC 없이 GNU 툴체인(MinGW)으로 빌드합니다.
   `rustup default stable-gnu` + MinGW-w64가 PATH에 필요합니다.

## English summary

- All contributions are accepted under the Apache License 2.0 (no CLA).
- Please sign off your commits (`git commit -s`, DCO).
- Keep `cargo clippy` / `cargo test` clean and source files under the
  line limits enforced by `cargo run -p xtask -- lines`.
- Windows builds use the GNU toolchain (MinGW); MSVC is not required.
