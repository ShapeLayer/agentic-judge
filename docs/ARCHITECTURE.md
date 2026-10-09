# 데이터 형식과 실행 규칙

CLI, stdio MCP, 루프백 HTTP API는 하나의 Rust 엔진을 공유합니다. 기본 문제풀이 UI는 바이너리에 내장하며 API와 같은 Origin에서 제공합니다. 설치·설정·워크스페이스·초안 관리 규칙은 [INSTALLATION.md](INSTALLATION.md)에 있습니다. storage_root와 runtime_root는 독립적으로 지정합니다. 실행기에 전달되는 것은 코드와 현재 테스트 입력뿐이며 저장소 전체를 컨테이너에 마운트하지 않습니다. 각 테스트를 실행할 때 컴파일 산출물을 새 작업 디렉터리로 복사합니다. 실행 후 작업 디렉터리를 제거합니다. 기본 실행 환경은 네트워크가 차단된 비특권 Podman 컨테이너입니다. container=false를 선택하면 신뢰하는 코드를 로컬 환경에서 실행할 수 있습니다.

## 저장 형식

```
storage_root/problems/<id>/
  manifest.toml
  statements/{legend,input,output,notes}.md
  statements/example.<number>.{in,out}
  tests/<suite>/<number>.{in,out,json}
  evaluate/checker.cpp
  answers/<uuid>/current.json
  answers/<uuid>/revisions/<revision>.json
  submissions/<uuid>.json
  reviews/<uuid>.json
  validator-runs/<uuid>.json
```

manifest에는 id/name/time_limit_ms/memory_limit_mb/tags/example_inputs/example_outputs/checker/suites/main_solution/validator 필드를 저장합니다. 기본 suite는 general입니다. checker는 tokens/exact/lines, testlib:<배포된 checker 이름>, 또는 ./evaluate/checker.cpp입니다. 정해·validator·generator는 모두 UUID로 식별하는 answer로 저장하여 버전을 관리합니다. 코드 내용·언어·expected를 변경하면 리비전이 증가하지만, 같은 내용을 다시 저장하면 증가하지 않습니다.

개별 JSON·TOML 파일을 저장할 때는 먼저 임시 파일에 내용을 기록한 뒤, rename으로 기존 파일을 교체합니다. HTTP/MCP/CLI의 call은 저장소의 advisory lock을 공유하므로 서로 다른 프로세스가 수행하는 쓰기 작업을 순차적으로 처리합니다. 실행 중 수동 파일 편집은 지원하지 않습니다. 정해와 validator, 제한 설정의 해시가 generation입니다. 재생성을 시작할 때 모든 테스트의 generation을 비웁니다. 모든 입력이 validator를 통과하고 정해 실행에 성공하여 출력이 생성된 경우에만 ready가 참입니다. 한 테스트라도 실패하면 문제 전체가 채점 불가 상태입니다. 테스트 입출력 파일의 직접 변경도 해시 비교로 검출합니다.

AI fingerprint는 현재 답안 리비전 또는 테스트 입출력·generation, manifest, 문제 본문에 의존합니다. 과거 기록은 삭제하지 않되 현재 fingerprint와 일치하는 것만 현재 의견 집계에 포함합니다. AI 검증은 실행 검증과 다르며 합격 인증이나 정확도 확률이 아닙니다. 검사 오류도 기록합니다. AI API에는 검증에 필요한 문제/코드/테스트 데이터가 전송됩니다. 프로바이더 연결은 사용자가 설정하며 기본값은 비어 있습니다.

## 실행 환경

기본 실행 레시피는 C, C++, Rust, Python, JavaScript, TypeScript, Java, Go, Ruby, PHP, Bash를 지원합니다. filename과 compile/run의 인수 배열(argv)을 설정하여 언어를 추가할 수 있습니다. 실행 중에는 필요한 파일을 다운로드하지 않습니다. Podman 이미지와 실행 바이너리·의존성은 온라인 준비 단계에 설치하고 이후 오프라인으로 사용합니다. Containerfile은 개발용 이미지를 기준으로 작성했습니다. 이미지 변경이 채점 결과에 영향을 줄 수 있으므로 배포 시에는 이미지 다이제스트 고정을 권장합니다.

컨테이너는 네트워크 차단, read-only root, capability 제거, no-new-privileges, PID/CPU/메모리 제한, 비특권 UID를 적용합니다. 컴파일도 컨테이너 내부에서 수행합니다. 런타임 출력 파일의 크기는 16 MiB로 제한하며, 실제 경과 시간(wall time)에는 문제에 설정된 시간 제한을 적용합니다. 현재는 Podman을 시작하는 시간도 실행 시간에 포함됩니다. 컨테이너의 OOMKilled 상태를 근거로 MLE를 판정합니다. 로컬 Linux는 주소 공간 제한을 적용합니다. 로컬 macOS에는 주소 공간의 하드 제한을 적용하지 않으며 최대 RSS를 기준으로 실행 후에만 판정할 수 있습니다. 따라서 적대적인 코드를 실행하거나 정확한 MLE 평가가 필요할 때는 컨테이너를 사용합니다. local 모드는 호스트 파일·네트워크를 격리하지 않습니다. Linux의 메모리 할당 실패가 사용자 프로그램에서 처리되면 RE 등으로 종료될 수 있습니다. 컨테이너 memory_kb=0은 RSS 측정값이 없음을 뜻합니다.

현재 엔진은 요청을 동기식으로 순차 처리하며, 로컬 환경에서 개인이 사용하는 용도로 설계되었습니다. 과거 제출 기록에는 코드·결과·generation과 각 실행의 입력·기대 출력 스냅샷을 보관합니다. 정해가 논리적으로 틀려도 실행에 성공하면 출력은 생성되므로 독립적인 AI 검토, 다른 AC 풀이와의 교차 확인, 반례 탐색이 중요합니다.

## 프로토콜

MCP 2025-11-25 규격에 따라 stdio로 JSON-RPC 메시지를 주고받습니다. tools/list, tools/call, prompts/list, prompts/get, ping, initialize 메서드를 지원합니다. stdout에는 프로토콜 메시지만 기록합니다. VS Code는 .vscode/mcp.json의 서버를 실행합니다. 워크스페이스 범위는 설정 workspace_root이며 workspace_read는 상대 경로나 심볼릭 링크를 통해 워크스페이스 밖에 접근하는 요청을 거부합니다. register_problem 프롬프트를 실행하는 MCP 클라이언트의 AI가 여러 원본 파일을 해석하고 정해/테스트를 작성합니다. 서버 자체는 AI 답안 자동수정이나 무단 자율 등록을 하지 않습니다.

HTTP API는 루프백에서만 접근할 수 있으며, POST /api/tools/<동일 MCP tool 이름>으로 요청합니다. Content-Type은 application/json이며, MCP와 동일한 JSON 인수와 도구 결과를 사용합니다. 오류는 HTTP 400의 error 문자열입니다. 외부 Origin을 거부합니다. 별도 UI 서버를 연결할 때는 같은 출처(origin)의 리버스 프록시를 통해 요청을 전달합니다. 인증 기능이 없는 루프백 API이므로 신뢰하지 않는 로컬 사용자가 있는 공유 호스트에는 그대로 노출하지 않습니다.

프로토콜과 서버 설정은 [MCP 공식 명세](https://modelcontextprotocol.io/specification/2025-11-25)와 [VS Code MCP 서버 문서](https://code.visualstudio.com/docs/agent-customization/mcp-servers)를 참고합니다. testlib.h 및 commons checker는 _PREWORK_REFS/testlib-master에서 복사했고 MIT 라이선스 원문을 vendor/testlib/LICENSE에 보관했습니다.

상대 경로 설정은 설정 파일이 위치한 디렉터리를 기준으로 해석합니다. 네이티브 실행 대상은 macOS/Linux이며 Windows에서는 WSL2를 사용합니다. 수식은 latex2mathml로 변환한 뒤 MathML 전용 허용 목록에 따라 정화하며 지원하지 않는 LaTeX는 원문으로 남깁니다. API 스키마는 API-TOOLS.json과 `agentic-judge schema`로 조회합니다.

Podman 전환: 기본 container_command는 podman, 이미지 이름은 localhost/agentic-judge-runtime:local입니다. rootless keep-id 매핑으로 호스트 사용자를 컨테이너 UID/GID 65534에 연결하여 컴파일 산출물의 소유권을 유지합니다. 작업 디렉터리 권한은 0700으로 설정하고, 바인드 마운트에는 SELinux private relabel을 적용합니다. run/inspect/kill/rm 모두 설정된 Podman 명령을 사용하며 Docker daemon/socket에 의존하지 않습니다. [Podman 실행 옵션 공식 문서](https://docs.podman.io/en/latest/markdown/podman-run.1.html)를 기준으로 구성했습니다.

AI 모델 이력: 각 Review에 requested_model(요청 시 설정된 모델)과 response_model(API 최상위 model 메타데이터)을 보관합니다. 요청 전 requested_model을 고정하므로 연결이나 인증에 실패해도 요청 모델 정보가 남고, 응답 본문 파싱 실패에도 수신한 response_model을 보존합니다. 모델의 자기소개는 식별 근거로 사용하지 않습니다. 메타데이터가 없는 응답은 요청 모델만 기록하며, 기존 기록의 누락 필드는 null로 읽고 현재 설정으로 소급 보충하지 않습니다.

## 외부 문서 가져오기

`problem_import(problem_id, path, format?, language?, time_limit_ms?, memory_limit_mb?)`는 문서 데이터를 기존 저장 형식으로 변환합니다. `format`은 `auto`(기본값), `polygon`, `boj_archive`입니다. 원본은 설정의 `workspace_root` 안에 있어야 하며 심볼릭 링크를 통한 범위 이탈도 거부합니다. 기존 ID는 덮어쓰지 않습니다. 변환과 파일 저장이 모두 성공한 뒤 디렉터리를 게시하므로 실패한 가져오기는 미완성 문제를 남기지 않습니다.

| 원본 | 대상 |
| --- | --- |
| Polygon 언어별 name / 백준 title | manifest.name |
| Polygon time-limit(ms), memory-limit(bytes) / 백준 time-limit(초), memory-limit(MB) | manifest.time_limit_ms / memory_limit_mb |
| Polygon tags/tag@value / 백준 tags 목록 | manifest.tags (문자열 배열, 생략 시 빈 배열) |
| Polygon legend/input-specification/output-specification/note | statements/legend/input/output/notes.md |
| 백준 문제/입력/출력 | statements/legend/input/output.md |
| 백준 힌트·조건·출처·서브태스크 등 나머지 문서 절 | 원래 제목과 함께 statements/notes.md |
| 언어별 Polygon sampleTests / 백준 예제 입력·출력 코드 블록 | statements/example.N.in/out |

Polygon은 압축을 푼 패키지 디렉터리 또는 `problem.xml`을 받습니다. HTML statement가 포함된 패키지를 지원하며, `language`를 생략하면 korean → english → 첫 번째 제공 언어 순서로 선택합니다. 지정 언어가 없거나 HTML이 없는 패키지는 명확한 오류를 반환합니다. LaTeX 수식, 기본 표·목록·링크를 Markdown으로 변환합니다. 예제는 `problem-properties.json`의 샘플 파일을 우선 읽어 공백을 보존하며 이 JSON이 없는 패키지는 HTML 예제를 사용하고 경고합니다. 로컬 PNG/JPEG/GIF/WebP 이미지는 `statements/assets/<sha256>.<확장자>`로 복사하고 `GET /api/problems/<id>/assets/<파일명>`으로 제공합니다. 외부 이미지 URL은 유지하며 다운로드하지 않습니다. 지원하지 않는 로컬 이미지 형식은 오류로 반환합니다.

백준 아카이브는 `problems/<번호>번_ <제목>.md` 파일 하나를 받습니다. 아카이브의 메타데이터는 제목에 따옴표 없이 콜론이 포함될 수 있으므로, 지원 필드의 줄 형식을 기준으로 읽습니다. 제목은 원문 문자열, tags는 YAML 문자열 목록입니다. 빈 예제 제목의 중복과 동일한 예제의 반복은 허용하되 서로 다른 데이터의 중복은 오류로 처리합니다. 원본에서 누락되거나 모호한 예제는 값을 추측하지 않고 남아 있는 본문을 notes에 보존하며 `warnings`로 알립니다. 완전한 예제 쌍만 1부터 연속 번호로 등록합니다. 예제 뒤 설명도 notes에 보존합니다. 빈 문제 본문도 경고로 알립니다.

제한값 뒤의 괄호 주석은 notes에 보존합니다. 원본 제한이 0 또는 ‘서브태스크 참고’처럼 단일 양수로 해석되지 않으면 해당 `time_limit_ms`/`memory_limit_mb`를 명시해야 합니다. 원본에 없는 제한을 자동으로 추측하지 않습니다. 오버라이드한 백준 원본 제한 역시 notes에 남깁니다. 레벨·티어 관련 기능이나 필드는 추가하지 않습니다.

반환값은 `{manifest, format, warnings, ready:false}`입니다. 이 도구는 문제 문서만 등록합니다. 원본 프로그램을 실행하거나 정해·validator·checker·비공개 테스트를 자동 등록하지 않으며, 채점 준비 상태는 기존 등록·검증 절차를 거쳐야 합니다.

### Checker reuse
Compiled C++ checkers (custom and built-in testlib) are cached per problem under `.checker-cache/`. The cache key includes source, testlib header, C++ compile/run configuration, runtime mode, container command/image, and application version. A file lock serializes cache publication across processes, and failed compilations do not publish entries. Submissions receive private working copies of the cached artifacts so test input/output never mutates the shared cache. Built-in tokens/exact/lines comparisons require no compilation. Submission `checker.txt` files contain per-case checker diagnostics, not checker source or binaries.
