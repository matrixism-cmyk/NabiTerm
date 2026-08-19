# 보안 정책 / Security Policy

## 취약점 신고 / Reporting a Vulnerability

보안 취약점은 공개 이슈 대신 아래로 신고해 주세요:

- **GitHub 비공개 신고**: [Security ▸ Report a vulnerability](https://github.com/matrixism-cmyk/NabiTerm/security/advisories/new) (권장)
- **이메일**: matrixism@gmail.com

Please report security vulnerabilities privately via GitHub's
"Report a vulnerability" (preferred) or by email — not as public issues.

- 접수 후 가능한 한 빠르게(영업일 기준 수일 내) 회신합니다.
- 수정 릴리스가 나올 때까지 상세 내용의 공개를 미뤄 주시면 감사하겠습니다(협조적 공개).

## 지원 버전 / Supported Versions

최신 릴리스만 보안 수정을 받습니다. 자동 업데이트로 항상 최신 버전을 유지해 주세요.
Only the latest release receives security fixes; auto-update keeps you current.

## 범위 참고 / Scope notes

- nabiTerm은 텔레메트리·외부 통신이 없으며(자동 업데이트 확인은 opt-in), 완전 오프라인에서 동작합니다.
- 릴리스 인스톨러는 릴리스 노트의 SHA-256과 대조 검증 후에만 실행됩니다.
- 저장소 히스토리의 `*_test.rs`에 포함된 SSH 개인키·자격증명 문자열은 인프로세스 테스트 서버 전용 픽스처로, 실서비스 가치가 없습니다.
