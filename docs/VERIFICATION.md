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
- UI는 요청에 따라 구현하지 않았습니다. UI-SPEC.md와 API-TOOLS.json으로 별도 작업자에게 인계합니다.
- Linux/WSL2 네이티브 실행은 이 머신에서 확인하지 않았습니다. Linux CI 설정은 작성했으며 원격 CI를 실제 실행한 것은 아닙니다.

현재 기본 컨테이너 모드에서 실행하려면 Podman과 준비된 runtime 이미지가 필요합니다. Podman 없이 확인하려면 신뢰하는 코드에 한해 judge.toml에서 container=false를 선택하고 필요한 호스트 언어 런타임을 설치합니다. macOS 로컬 메모리 초과는 사후 RSS 판정이므로 강제 메모리 격리가 필요한 경우 Podman을 사용합니다.

## Podman 전환

위 초기 검증의 Docker 관련 항목은 당시 이력입니다. 현재 기본 설정·로컬 judge.toml·Containerfile·컨테이너 테스트·CI는 Podman으로 전환했습니다. 이 머신에는 Podman도 설치되어 있지 않으므로 실제 Podman 컨테이너 테스트는 실행하지 않았습니다.
