# 설치 및 실행 안내

## 경로와 설정 우선순위

소스 빌드에는 Node.js 22 이상과 npm이 필요하며, 최초 `npm ci --prefix ui`를 실행해야 합니다. Cargo는 TypeScript UI를 자동 컴파일합니다.

설치 파일은 읽기 전용으로 취급합니다. `cargo install --path . --locked` 또는 추출한 배포 아카이브의 `install.sh`로 실행 파일을 설치합니다. 배포 설치에는 Rust·소스 저장소·별도 UI 서버가 필요하지 않습니다.

| 항목 | macOS 기본값 | Linux/WSL2 기본값 |
|---|---|---|
| 설정 | `~/Library/Application Support/Agentic Judge/judge.toml` | `$XDG_CONFIG_HOME/agentic-judge/judge.toml` |
| 영구 데이터 | `~/Library/Application Support/Agentic Judge/data` | `$XDG_DATA_HOME/agentic-judge` |
| 실행 임시 공간 | `~/Library/Caches/Agentic Judge/run` | `$XDG_CACHE_HOME/agentic-judge/run` |
| 문제풀이 폴더 | 선택 사항 | 선택 사항 |

XDG 변수가 없거나 상대 경로이면 `~/.config`, `~/.local/share`, `~/.cache`를 사용합니다. `AGENTIC_JUDGE_HOME=/absolute/path`는 위 기본값을 `config/judge.toml`, `data`, `cache/run`으로 대체합니다. 이를 설정한 경우에만 설치형 데이터를 한 폴더에 모읍니다. 상대 경로는 거부합니다.

1. `--config` 또는 기존 위치 인자로 지정한 설정을 읽습니다. 없으면 사용자 공통 설정을 생성하고 읽습니다.
2. 현재 디렉터리의 `judge.toml`은 자동 탐색하지 않습니다. 문제풀이 저장소의 설정이 실행 명령·AI endpoint를 바꾸지 못하게 합니다.
3. 설정의 상대 경로는 그 파일의 부모 디렉터리 기준입니다.
4. `--workspace`는 설정의 workspace_root를 덮어씁니다. 상대 경로는 실행 디렉터리 기준입니다. 디렉터리 존재 여부를 확인하고 정규화합니다.
5. `--scratch`는 workspace_root를 해제합니다. `--workspace`와 동시에 지정할 수 없습니다.
6. workspace_root가 없으면 파일 탐색·조회·파일 기반 제출·문서 가져오기는 거부합니다. source 기반 제출·문제 관리·초안은 그대로 사용할 수 있습니다.

기존 명시적 설정의 `storage_root='data'` 등은 그대로 유지됩니다. `init-config`로 생성하는 설정만 사용자 기본 절대 경로를 사용합니다. 라이브러리 `Config::default()`를 직접 쓰는 코드에서는 storage/run 경로를 명시적으로 덮어쓰는 것을 권장합니다. 설치 경로 해석은 `config::load`의 책임입니다.

## 데이터와 세션

기본 문제·정해·테스트·제출 이력은 사용자 공통 라이브러리입니다. 폴더마다 다른 문제 라이브러리가 필요하면 별도 설정 파일의 storage_root를 지정합니다. runtime_root와 storage_root는 서로 포함하지 않는 독립 디렉터리여야 합니다.

한 엔진 프로세스는 한 워크스페이스 또는 scratch 상태를 갖습니다. 폴더 전환은 프로세스를 새 옵션으로 재실행합니다. 워크스페이스 루트를 임의의 API 호출로 바꾸는 기능은 제공하지 않습니다. 여러 창에서는 별도 프로세스·별도 포트를 사용합니다. HTTP와 MCP는 같은 사용자 설정을 이용해 문제 라이브러리를 공유할 수 있으며, 기존 파일 잠금이 쓰기를 직렬화합니다.

`draft_get` / `draft_save`는 `(정규화된 workspace_root 또는 null, problem_id, language)`의 해시를 키로 `storage_root/drafts/`에 저장합니다. 초안은 정해나 제출이 아닙니다. 즉석 작성 초안은 서버 재시작·포트 변경 뒤에도 복구됩니다. 동일 문제·언어·워크스페이스를 여러 창에서 수정하면 마지막 저장이 적용됩니다.

웹 에디터는 입력 직후 브라우저에도 미전송 초안을 기록하고 서버에 자동 저장합니다. 서버 저장 완료 전 페이지를 닫으면 경고합니다. 새로고침 때 미전송 초안을 우선 복구합니다. 브라우저 저장이 막힌 환경에서는 서버 저장과 파일 다운로드를 사용합니다. 언어별 초안은 선택한 언어로 전환하면 복구됩니다. 파일로 저장은 초안 저장 후 동일 Origin의 `GET /api/draft-download?problem_id=...&language=...`에서 UTF-8 파일을 다운로드합니다. 응답은 attachment와 no-store 헤더를 포함하며 워크스페이스 파일을 덮어쓰지 않습니다.

## 런타임 설치와 진단

`runtime-install build`는 바이너리에 포함된 Containerfile을 임시 디렉터리에 꺼내고 Podman으로 빌드한 뒤 정리합니다. 기본 태그는 `localhost/agentic-judge-runtime:<앱 버전>`입니다. 기존 설정에 `:local`이 있으면 그 태그를 그대로 사용합니다.

- `runtime-install load /absolute/image.tar`: 로컬 OCI/Docker 이미지 아카이브 설치. 설치 후 설정의 이미지가 존재하는지 검사합니다.
- `runtime-install pull`: 설정에 지정된 이미지 다운로드. 기본 localhost 태그 대신 실제 배포 레지스트리 주소를 지정해야 합니다.
- `doctor`: Podman 연결, 이미지 존재, 동일한 UID 매핑과 runtime_root 마운트에서 파일 읽기·쓰기를 확인합니다. 실패 시 종료 코드는 0이 아니며 조치를 JSON으로 출력합니다.
- `container=false`에서 doctor는 설정된 언어의 컴파일러·실행 파일이 PATH에서 실행 가능한지 확인합니다. 실제 언어별 프로그램의 정상 동작까지 보증하지 않습니다.

macOS에서는 Podman VM을 사용자가 준비해야 합니다. runtime_root를 VM 공유 경로 안에 두세요. 초기 이미지 빌드는 네트워크가 필요하고, 이후 채점은 기존과 같이 `--network=none --pull=never`로 실행됩니다. Linux는 rootless Podman의 UID 매핑과 cgroup 환경이 필요합니다. 네이티브 Windows는 지원하지 않고 WSL2에서 설치합니다.

## UI와 서버

`agentic-judge`만 실행하면 기본 루프백 주소로 서버를 시작하고 브라우저를 엽니다. `serve`는 `--open`을 추가할 때만 브라우저를 엽니다. 종료는 실행 중인 터미널에서 Ctrl+C입니다. 기본 포트가 사용 중이면 `--listen 127.0.0.1:0` 또는 다른 포트를 지정합니다. 자동 배정된 실제 주소는 표준 오류로 출력하며, MCP 표준 출력에는 프로토콜만 기록합니다.

UI 자원과 API는 같은 Origin입니다. 앱·스크립트·CSS는 바이너리에 내장하며 런타임에 ui/ 폴더를 읽지 않습니다. Host와 API Origin을 실제 바인딩 주소와 대조하고 외부 접근을 거부합니다. 별도 개발용 정적 UI 서버와 연결하기 위해 CORS를 풀지 않습니다.

설치형 화면은 문제풀이 흐름과 기본 채점 준비를 제공합니다. 에디터는 오프라인 기본 textarea입니다. 현재 기능 범위와 API 연결은 [UI-SPEC.md](UI-SPEC.md), 배치는 [UI-DESIGN.md](UI-DESIGN.md)에 기록되어 있습니다.

## 배포와 업데이트

`scripts/package.sh`는 빌드된 바이너리, installer, README, testlib 라이선스를 플랫폼 이름의 tar.gz와 SHA-256 파일로 묶습니다. 설치 스크립트는 실행 파일만 교체하며 사용자 데이터·설정을 덮어쓰지 않습니다. `Cargo.toml`의 패키지 include 목록은 참고자료·사용자 데이터를 소스 배포에서 제외합니다.

`release.yml`은 Linux·macOS 바이너리와 Linux amd64 런타임 아카이브를 CI artifact로 만듭니다. 공개 레지스트리나 GitHub Release 게시를 자동 수행하지 않습니다. 런타임 아카이브는 CPU 아키텍처에 맞는 것을 사용하세요. Apple Silicon에서는 내장 레시피로 빌드하면 해당 환경의 아키텍처를 사용합니다.

기존 저장소 데이터는 자동 이동하지 않습니다. 이전 judge.toml을 `--config`로 지정하면 바로 계속 사용할 수 있습니다. 사용자 공통 경로로 옮기려면 서버를 종료하고 기존 data를 백업한 뒤 새 storage_root로 복사하고 설정을 변경합니다. 앱 업데이트와 데이터 이전은 별개입니다.

## 검증

`tests/installed.rs`는 실행 파일을 임시 설치 폴더로 복사하고 다른 작업 디렉터리에서 실행합니다. 첫 실행, 동시 초기화, 기존 설정, 외부 폴더의 파일 제출, scratch 제출, 폴더별 초안과 재시작 복구, 경로 이탈 거부, MCP 프롬프트, 내장 UI 제공, Host·Origin 경계, 레시피 추출과 설치 실패 전파를 확인합니다. 모의 컨테이너 실행기는 설치 명령의 동작 규칙만 검증하며 실제 Podman 실행을 대신하지 않습니다. 실제 런타임 검증은 CI의 `runtime-install`, `doctor` 및 기존 컨테이너 테스트가 담당합니다.
