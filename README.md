# Agentic Judge

Agentic Judge는 로컬 환경에서 오프라인으로 문제를 제작하고 채점하는 Rust 엔진입니다. VS Code MCP와 HTTP API를 제공하며, 프론트엔드 구현에 필요한 [UI 기능 명세](docs/UI-SPEC.md)와 [디자인 명세](docs/UI-DESIGN.md), 정적 디자인 시스템·목업([ui/](ui/README.md))을 제공합니다.

## 시작

```sh
cargo build --release --locked
cp config/judge.example.toml judge.toml
podman build -t localhost/agentic-judge-runtime:local runtime
./target/release/agentic-judge serve judge.toml
```

빌드 의존성과 컨테이너 이미지를 준비한 뒤에는 채점에 인터넷이 필요하지 않습니다. Podman이 없으면 judge.toml의 container를 false로 바꾸고 해당 언어 런타임을 호스트에 설치합니다. 로컬 모드는 신뢰하는 코드 전용입니다. storage_root, runtime_root, workspace_root는 독립적으로 지정합니다. 기본 HTTP 주소는 127.0.0.1:27121입니다.

VS Code에서 저장소를 열면 `.vscode/mcp.json`의 agentic-judge 서버를 시작할 수 있습니다. 바이너리 빌드와 judge.toml 준비가 먼저 필요합니다. register_problem, audit_problem, learning_hint 프롬프트를 사용합니다. 다른 작업 폴더에서 사용하려면 command/config/workspace_root를 절대 경로로 설정합니다.

```sh
./target/release/agentic-judge call judge.toml problem_create '{"problem_id":"sum","name":"A+B"}'
./target/release/agentic-judge call judge.toml answer_save '{"problem_id":"sum","language":"python","source":"a,b=map(int,input().split()); print(a+b)","expected":"AC","main":true}'
./target/release/agentic-judge call judge.toml test_put '{"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}'
./target/release/agentic-judge call judge.toml submit '{"problem_id":"sum","language":"python","source":"print(sum(map(int,input().split())))"}'
```

전체 기본 언어 설정은 `agentic-judge init-config another.toml`로 생성할 수 있습니다. 여러 AI 프로바이더를 사용하려면 설정 파일의 providers 테이블에 각 프로바이더를 추가합니다. OpenAI 호환, Anthropic, Ollama 프로토콜 어댑터를 제공하며, AI 프로바이더에 연결하지 않아도 채점할 수 있습니다. AI는 타당성 판단과 수정 방향만 기록하며 파일을 변경하지 않습니다.

## 개발·검증

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

통합 테스트를 실행하려면 로컬 환경에 Python 3와 C++ 컴파일러가 필요합니다. [데이터·실행 계약과 제한](docs/ARCHITECTURE.md), [UI 명세](docs/UI-SPEC.md)를 참고합니다. testlib.h와 checker 코드는 vendor/testlib에 포함되어 있습니다. `_PREWORK_REFS`는 원본 참고자료이며 수정하지 않습니다.

API 요청 스키마는 [API-TOOLS.json](docs/API-TOOLS.json)에 있습니다. 네이티브 macOS/Linux를 지원하며 Windows는 WSL2에서 실행합니다. 설정의 상대 경로는 설정 파일이 위치한 디렉터리를 기준으로 합니다.

Podman 실행 검증은 이미지를 준비한 뒤 `cargo test --test container -- --ignored`로 수행합니다. 이 테스트는 11개 언어의 실행, OOM에 의한 MLE, TLE, 네트워크 차단을 확인하며 기본 테스트에서는 건너뜁니다. CI는 이미지 빌드와 이 테스트를 별도로 실행하도록 구성했습니다.

Podman은 일반 사용자(rootless)로 실행합니다. macOS에서는 최초 한 번 `podman machine init`을 실행한 뒤 `podman machine start`로 Linux VM을 시작합니다. runtime_root는 Podman VM에 공유된 호스트 경로여야 합니다. Linux에서는 cgroup v2와 사용자 namespace(subuid/subgid)가 설정된 환경을 사용합니다. 실행 시 이미지는 자동으로 내려받지 않으므로 위의 이미지 빌드가 먼저 필요합니다.
