# 검증 결과 (2026-09-18)

macOS / Rust 1.94.0에서 확인했습니다.

- `cargo test --offline --locked`: 통합 테스트 10개가 통과했습니다. Docker 전용 테스트 1개는 ignored로 명시하여 실행에서 제외했습니다.
- `cargo clippy --offline --locked --all-targets -- -D warnings`: 통과했습니다.
- `cargo fmt --check`: 통과했습니다.
- `cargo build --offline --release --locked`: 통과했습니다. target/release/agentic-judge 바이너리를 생성했습니다.
- 기본 judge.toml 설정을 생성했고 release 바이너리의 capabilities 호출을 확인했습니다.

실제 실행으로 다음 항목을 확인했습니다.

- 채점 결과: Python의 AC/WA/RE/TLE 판정과 macOS의 최대 RSS 초과에 따른 MLE 판정을 확인했습니다.
- 테스트 도구: C++ testlib generator, custom checker, 내장 ncmp checker를 확인했습니다.
- 재생성과 입력 검증: 정해 리비전 변경 후 출력 재생성, validator 실패 시 준비 상태 해제, validator의 무효 입력 거부 기록을 확인했습니다.
- 파일 격리: 테스트 간 파일 격리와 정리, workspace 밖에 접근하는 경로의 거부를 확인했습니다.
- 본문 렌더링: 수식의 MathML 변환과 HTML 정화를 확인했습니다.
- AI 검증: 정상 응답과 잘못된 JSON 응답의 기록, 답안 코드가 수정되지 않는 동작, 과거 리비전의 AI 기록이 현재 집계에서 제외되는 동작을 확인했습니다.
- 프로토콜: stdio MCP와 프롬프트, HTTP JSON API와 외부 Origin 차단을 확인했습니다.

미검증 환경:

- Docker가 이 머신에 없어 컨테이너의 11개 언어 실행·cgroup OOM·네트워크 차단 테스트는 실행하지 않았습니다. tests/container.rs와 CI에 해당 검증이 준비되어 있습니다. 이미지 빌드 후 `cargo test --test container -- --ignored`로 실행합니다.
- 실제 외부 AI 키/모델은 설정하지 않았습니다. OpenAI 호환 어댑터는 loopback 모의 서버로 검증했고 Anthropic/Ollama 어댑터의 실제 서비스 연결은 확인하지 않았습니다.
- Linux/WSL2 네이티브 실행은 이 머신에서 확인하지 않았습니다. Linux CI 설정은 작성했으며 원격 CI를 실제 실행한 것은 아닙니다.

현재 기본 컨테이너 모드에서 실행하려면 Podman과 준비된 runtime 이미지가 필요합니다. Podman 없이 확인하려면 신뢰하는 코드에 한해 judge.toml에서 container=false를 선택하고 필요한 호스트 언어 런타임을 설치합니다. macOS 로컬 메모리 초과는 사후 RSS 판정이므로 강제 메모리 격리가 필요한 경우 Podman을 사용합니다.

## Podman 전환

위 초기 검증의 Docker 관련 항목은 당시 이력입니다. 현재 기본 설정·로컬 judge.toml·Containerfile·컨테이너 테스트·CI는 Podman으로 전환했습니다. 이 머신에는 Podman도 설치되어 있지 않으므로 실제 Podman 컨테이너 테스트는 실행하지 않았습니다.

## 문서 가져오기 검증

- 새 `tests/import.rs` 8개 테스트 통과: 두 포맷의 메타데이터·태그·예제 매핑, 수식·표·이미지, 빈 예제 제목 중복, 불완전한 예제 보존, 제한값 오버라이드, 기존 ID 보호, 원본 경로 이탈 거부, HTTP 이미지 응답·외부 Origin·심볼릭 링크 차단을 확인했습니다.
- 기존 일반 테스트와 통합 테스트 11개 통과. 루프백 포트를 여는 테스트는 샌드박스의 포트 제한 밖에서 실행했습니다. Podman 전용 테스트는 이 변경의 검증 범위에서 제외했습니다.
- `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check` 통과.
- 실제 `_PREWORK_REFS` Polygon 23개와 백준 아카이브 Markdown 33,455개를 전수 파싱했습니다. 백준 77개는 원본에 유효한 단일 제한이 없어 명시적 제한값을 전달하는 경로로 검증했습니다. 검증용 제한값으로 실제 사용자 문제를 등록하지 않았습니다. 불완전·모호한 예제 및 빈 본문 등은 경고로 드러납니다. 이 전수 검사는 파싱 검사이며 원본 문제의 정확성이나 전체 화면의 시각적 일치를 보증하지 않습니다.

로컬 참조자료 전수 검사는 다음과 같이 재현할 수 있습니다. `BOJ_ARCHIVE`는 아카이브의 `problems` 디렉터리입니다.

```sh
BOJ_ARCHIVE=/path/to/baekjoon-online-judge-problem-archive/problems cargo test --locked parse_local_reference_documents -- --ignored --nocapture
```


## 설치형 실행 전환 검증

2026-09-18, macOS arm64에서 확인했습니다.

- `cargo test --locked`: 일반 단위 테스트 2개, 가져오기 8개, 설치형 실행 9개, 기존 통합 테스트 11개 — 총 30개 통과. 로컬 HTTP 포트가 필요한 테스트는 샌드박스 밖에서 실행했습니다.
- `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `node --check ui/app/app.js` 통과.
- 설치형 테스트는 바이너리를 임시 설치 폴더로 복사해 별도 작업 디렉터리에서 실행합니다. 사용자 설정 자동 생성·동시 초기화·기존 설정·scratch·외부 workspace 파일 제출·초안 분리/복구·MCP·내장 UI·Host/Origin·진단 실패와 런타임 설치 명령의 동작 규칙을 확인했습니다.
- `scripts/package.sh`로 `dist/agentic-judge-0.1.0-darwin-arm64.tar.gz`와 SHA-256 파일을 생성했습니다. 추출한 installer를 `/tmp`의 별도 설치 위치에 실행하고 capabilities·내장 UI 동작을 확인했습니다.
- 브라우저에서 문제 생성 → 설명·정해·테스트 등록 → Python·C++ 제출 AC → 새로고침 후 언어·초안·제출 이력 복구를 확인했습니다. 파일 열기와 언어 자동 선택도 확인했습니다. 초안 파일 다운로드는 HTTP 응답의 원문 바이트·파일명·캐시 금지·외부 Origin 차단을 통합 테스트로 확인했고, 브라우저 다운로드 완료 이벤트도 확인했습니다.
- `cargo package --list --allow-dirty --offline` 출력에서 UI·프롬프트·testlib·Containerfile 포함 및 `_PREWORK_REFS`, data, run 제외를 확인했습니다.

실제 Podman VM은 이 머신에 없으므로 컨테이너 이미지 빌드/마운트 실행은 로컬에서 검증하지 않았습니다. `runtime-install`의 build/pull/load는 모의 실행기로 성공·실패 전파를 확인했으며, 실제 검증은 CI에 구성했습니다. Linux 배포·원격 CI 실행도 이 로컬 검증의 결과에 포함하지 않습니다. 참고자료 전수 파싱과 Podman 전용 테스트는 이번 일반 테스트에서 ignored 상태입니다.

## TypeScript 전환 검증

- 실제 앱의 JavaScript 소스를 TypeScript로 전환했습니다. `ui/tsconfig.json`은 `strict`와 `noEmitOnError`를 사용합니다.
- `npm ci --prefix ui`, `npm run check --prefix ui`, `npm run build --prefix ui` 통과. 출력은 `ui/dist/`이며 Cargo 빌드가 실제 앱 스크립트를 컴파일하여 내장합니다.
- `cargo test --locked`: 30개 통과, 선택적 참고자료 테스트와 Podman 테스트 2개는 기존 설정대로 제외.
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` 통과.
- 헤드리스 Chrome: 내장 앱의 문제 생성, 초안 저장·새로고침 후 복구, 에디터 Tab 입력 확인. 페이지 JavaScript 오류와 실패한 리소스 요청 없음.

## UI 문서와 자산 정리 검증

- 배포 앱이 사용하지 않는 디자인 페이지·샘플 데이터·Web Components·전용 CSS를 제거했습니다. 앱이 참조하던 타입은 `ui/app/types.ts`로 옮기고 TypeScript 빌드 대상을 `app/`으로 한정했습니다.
- UI-SPEC·UI-DESIGN·UI README를 현재 내장 앱의 기능·배치·빌드 문서로 갱신했습니다. API-TOOLS.json은 현재 `schema` 출력과 일치하여 유지했습니다.
- `npm run check --prefix ui`, `npm run build --prefix ui` 통과. 문서의 로컬 링크와 `git diff --check`를 확인했습니다.
- `cargo test --offline --locked --test installed installed_http_serves_embedded_app_and_preserves_boundaries` 통과. 내장 HTML·JS·CSS와 HTTP 접근 경계를 확인했습니다. 최초 샌드박스 실행은 서버 시작 단계에서 실패했으며, 로컬 포트를 허용한 실행에서 통과했습니다. 이번 정리는 브라우저 시각 검사를 다시 수행하지 않았습니다.

## AI 없는 독립 실행 평가

[AI-INDEPENDENCE.md](AI-INDEPENDENCE.md)에 기능별 판정·코드 근거·실행 검증과 제한을 기록했습니다. 가져오기·설치형·통합 테스트 28개가 통과했으며, AI 미설정 채점과 AI 키 누락 오류 이후 채점·초안 기능을 확인했습니다. AI 요청 중 저장소 전체 잠금과 순차 HTTP 처리로 인한 일반 기능 지연은 남아 있습니다.
