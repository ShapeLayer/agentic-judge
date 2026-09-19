# Agentic Judge

Agentic Judge는 문제를 제작하고 채점하는 로컬 애플리케이션입니다. 설치된 실행 파일 하나에 문제풀이 웹 화면, MCP 서버, 프롬프트와 testlib 리소스가 포함됩니다. 문제풀이 중에는 이 소스 저장소를 열 필요가 없습니다.

## 설치와 시작

배포 아카이브를 풀고 그 안에서 설치합니다. Rust는 필요하지 않습니다.

```sh
sh install.sh                         # 기본 ~/.local/bin
# sh install.sh --prefix /absolute/path
export PATH="$HOME/.local/bin:$PATH"
agentic-judge                         # 공통 설정 자동 생성 + 브라우저 열기
```

소스에서 설치할 때만 `cargo install --path . --locked`를 사용합니다. 배포 아카이브는 `scripts/package.sh`로 만듭니다. Git 태그 또는 수동 실행으로 `.github/workflows/release.yml`이 플랫폼별 설치 파일을 생성합니다.

```sh
agentic-judge serve --workspace /path/to/solutions --open
agentic-judge serve --scratch --open
```

`--workspace`는 파일 조회·가져오기·파일 제출을 허용할 문제풀이 폴더입니다. 지정하지 않으면 기본적으로 즉석 작성 모드입니다. `--scratch`는 설정 파일에 지정한 폴더도 해제합니다. 폴더를 바꾸려면 서버를 종료하고 새 `--workspace`로 실행합니다. 동시에 여러 폴더를 열 때는 `--listen 127.0.0.1:0`으로 빈 포트를 배정받을 수 있습니다.

브라우저에서 문제 생성·선택, 정해·테스트 등록, 코드 작성·파일 열기·제출, 결과 조회와 파일 저장을 지원합니다. 초안은 문제·언어·워크스페이스별로 자동 저장됩니다. 문제와 제출 이력은 사용자 공통 라이브러리입니다. 별도 라이브러리는 명시적 설정의 `storage_root`로 선택합니다. 초기 UI 로딩과 준비된 런타임의 채점에는 인터넷이 필요하지 않습니다.

## 채점 런타임 준비

Podman 설치 후 다음을 실행합니다. macOS에서는 최초 한 번 `podman machine init`, 이후 `podman machine start`가 필요합니다.

```sh
agentic-judge runtime-install          # 내장 Containerfile로 버전별 이미지 빌드
agentic-judge doctor                   # 이미지·실행·VM 공유 경로 확인
```

이미지 최초 빌드는 네트워크가 필요합니다. 오프라인 설치는 해당 CPU 아키텍처의 이미지를 `podman save`로 가져와 `agentic-judge runtime-install load /path/to/runtime.tar`로 설치합니다. 미리 배포한 레지스트리 이미지를 쓸 때는 설정의 `image`를 지정하고 `runtime-install pull`을 실행합니다. 채점 중에는 이미지를 자동 다운로드하지 않습니다.

Podman 대신 호스트 언어 런타임을 사용하려면 설정에서 `container=false`로 바꿉니다. 이 모드는 신뢰하는 코드 전용입니다. `doctor`가 설정된 언어의 실행 파일을 확인합니다.

## 설정과 MCP

첫 실행은 사용자별 설정·데이터·캐시 디렉터리를 자동 생성합니다. macOS는 `~/Library/Application Support/Agentic Judge`와 `~/Library/Caches/Agentic Judge`, Linux/WSL2는 XDG 디렉터리를 사용합니다. `AGENTIC_JUDGE_HOME`에 절대 경로를 지정하면 그 아래 `config/`, `data/`, `cache/`로 통합합니다. 설치 디렉터리와 현재 폴더에는 데이터를 쓰지 않습니다.

현재 폴더의 `judge.toml`은 자동으로 읽지 않습니다. 기존 설정은 `agentic-judge serve --config /path/to/judge.toml` 또는 기존 형식 `agentic-judge serve /path/to/judge.toml`로 사용합니다. 설정 내부의 상대 경로는 설정 파일 기준이며 `--workspace`의 상대 경로는 명령 실행 폴더 기준입니다. `agentic-judge init-config /path/to/custom.toml`은 기존 파일을 덮어쓰지 않습니다.

VS Code에서는 문제풀이 폴더의 `.vscode/mcp.json`에 다음을 사용합니다. 실행 파일이 PATH에 없다면 `command`에 설치된 바이너리의 절대 경로를 지정합니다.

```json
{
  "servers": {
    "agentic-judge": {
      "type": "stdio",
      "command": "agentic-judge",
      "args": ["mcp", "--workspace", "${workspaceFolder}"]
    }
  }
}
```

폴더 없는 클라이언트는 `args: ["mcp", "--scratch"]`로 연결합니다. `register_problem`, `audit_problem`, `learning_hint` 프롬프트를 제공합니다. 여러 AI 프로바이더는 사용자 설정의 `providers`에 등록합니다. OpenAI 호환·Anthropic·Ollama를 지원하며 AI 연결 없이도 채점할 수 있습니다. AI는 타당성 판단과 수정 방향만 기록하고 파일을 변경하지 않습니다.

```sh
agentic-judge call problem_create '{"problem_id":"sum","name":"A+B"}'
agentic-judge call answer_save '{"problem_id":"sum","language":"python","source":"print(sum(map(int,input().split())))","expected":"AC","main":true}'
agentic-judge call test_put '{"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}'
agentic-judge call submit '{"problem_id":"sum","language":"python","source":"print(sum(map(int,input().split())))"}'
```

자세한 경로·배포·이전 방법은 [설치 안내](docs/INSTALLATION.md)에 있습니다. 현재 화면의 기능은 [UI-SPEC.md](docs/UI-SPEC.md), 화면 구성은 [UI-DESIGN.md](docs/UI-DESIGN.md), UI 빌드 방법은 [ui/](ui/README.md)에 있습니다.

## 개발·검증

```sh
npm ci --prefix ui
npm run check --prefix ui
cargo test --locked
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

통합 테스트를 실행하려면 로컬 환경에 Python 3와 C++ 컴파일러가 필요합니다. [데이터 형식·실행 규칙과 제한](docs/ARCHITECTURE.md), [UI 명세](docs/UI-SPEC.md)를 참고합니다. testlib.h와 checker 코드는 vendor/testlib에 포함되어 있습니다. `_PREWORK_REFS`는 원본 참고자료이며 수정하지 않습니다.

API 요청 스키마는 [API-TOOLS.json](docs/API-TOOLS.json)에 있습니다. 네이티브 macOS/Linux를 지원하며 Windows는 WSL2에서 실행합니다. 설정의 상대 경로는 설정 파일이 위치한 디렉터리를 기준으로 합니다.

Podman 실행 검증은 이미지를 준비한 뒤 `cargo test --test container -- --ignored`로 수행합니다. 이 테스트는 11개 언어의 실행, OOM에 의한 MLE, TLE, 네트워크 차단을 확인하며 기본 테스트에서는 건너뜁니다. CI는 이미지 빌드와 이 테스트를 별도로 실행하도록 구성했습니다.

Podman은 일반 사용자(rootless)로 실행합니다. macOS에서는 최초 한 번 `podman machine init`을 실행한 뒤 `podman machine start`로 Linux VM을 시작합니다. runtime_root는 Podman VM에 공유된 호스트 경로여야 합니다. Linux에서는 cgroup v2와 사용자 namespace(subuid/subgid)가 설정된 환경을 사용합니다. 실행 시 이미지는 자동으로 내려받지 않으므로 `runtime-install`이 먼저 필요합니다.

## Polygon·백준 아카이브 문서 가져오기

`problem_import`로 문제 설명, 시간·메모리 제한, 예제와 태그를 읽습니다. `tags`는 manifest의 기존 문자열 배열 필드입니다.

```sh
agentic-judge call --workspace /path/to/solutions problem_import '{"problem_id":"polygon-example","format":"polygon","path":"sources/polygon-package","language":"korean"}'
agentic-judge call --workspace /path/to/solutions problem_import '{"problem_id":"boj-1000","format":"boj_archive","path":"sources/baekjoon-online-judge-problem-archive/problems/1000번_ A+B.md"}'
```

경로는 `workspace_root` 기준입니다. 다른 위치의 아카이브를 읽으려면 해당 위치를 포함하도록 이 설정을 지정합니다. Polygon ZIP은 먼저 풀어 두어야 합니다. 가져오기는 새 ID에 문서만 등록하며 정해·채점 테스트는 별도로 준비합니다. 원본의 불완전한 예제 등은 반환된 `warnings`를 확인하세요. 단일 시간·메모리 제한이 없는 원본에는 `time_limit_ms`/`memory_limit_mb`를 명시할 수 있습니다. 자세한 매핑은 [외부 문서 가져오기 규칙](docs/ARCHITECTURE.md#외부-문서-가져오기)에 있습니다.

소스에서 빌드할 때는 Node.js 22 이상과 npm이 필요합니다. 최초 `npm ci --prefix ui` 후 Cargo가 TypeScript UI를 자동 빌드하여 바이너리에 내장합니다. UI 개발 방법은 [ui/README.md](ui/README.md)를 참고하세요.

### ZIP 내보내기 / 가져오기

- 문제 화면의 **문제 ZIP 내보내기**는 설명, 예제, 이미지, 채점 설정, 풀이와 검증기, 사용자 체커, 모든 테스트 셋을 함께 다운로드합니다. 사이드바의 **문제 ZIP 가져오기**에서 새 문제 ID로 복원합니다.
- **채점 준비 → 테스트 셋과 테스트**에서 셋 전체 또는 개별 테스트를 ZIP으로 다운로드합니다. 가져올 때 대상 셋 이름과 (개별 테스트의 경우) 번호를 지정합니다.
- 기존 문제·셋·테스트는 덮어쓰지 않습니다. 빈 테스트 셋도 지원합니다. 제출 기록과 풀이 초안은 문제 ZIP에 포함하지 않습니다.
- 가져오기는 코드를 실행하지 않습니다. 입력과 기대 출력은 보존하지만 검증 완료 상태는 초기화합니다. 정해가 등록되어 있다면 **기대 출력 다시 생성**으로 채점 준비를 완료하세요.
- ZIP은 버전 1의 `package.json`을 포함하는 앱 전용 형식입니다. 업로드 및 압축 해제 데이터 제한은 각각 32 MiB입니다. 기존 Polygon/BOJ 문서 가져오기도 계속 사용할 수 있습니다.
- HTTP: `GET /api/archive?kind=problem|suite|test&problem_id=...&suite=...&number=...`은 ZIP 첨부파일을 반환합니다. 동일 주소로 `Content-Type: application/zip`과 ZIP 본문을 POST하면 가져옵니다. `suite`는 셋/테스트에, `number`는 개별 테스트에 필요합니다.
