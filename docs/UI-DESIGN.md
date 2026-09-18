# 프론트엔드 디자인 명세

[UI-SPEC.md](UI-SPEC.md)는 UI가 제공할 기능을 정의하고, 이 문서는 화면의 시각적 구성과 표현 방식을 정의합니다. 구현 자산은 [ui/](../ui/README.md)에 있고, 이 문서와 `ui/styles/tokens.css`가 불일치하면 토큰 파일이 우선합니다.

## 원칙

1. **절제**: 색은 의미가 있을 때만 사용합니다. 기본 화면은 무채색 + 1px 경계선이며 강조색(accent)은 주 동작 버튼·포커스·링크에만 사용합니다. 그림자는 팝오버에만 사용합니다.
2. **좁은 여백**: 2px를 기준으로 간격 척도(2/4/6/8/12/16/24/32)를 사용합니다. 표 행의 높이는 28px, 컨트롤은 26px, 상단바는 36px로 설정합니다. 페이지 패딩과 카드 사이 간격은 12px로 설정합니다. 영역은 가는 경계선과 배경색 차이(`--c-bg-subtle`)로 구분합니다.
3. **텍스트 우선**: 아이콘만 있는 컨트롤은 최소화하고, 있으면 `aria-label`을 추가합니다. verdict·상태는 글리프 + 텍스트 + `title`로 전달하고 색은 보조 수단입니다.
4. **밀도 있는 정보**: 숫자에는 tabular-nums를 적용하고 단위를 함께 표기합니다(`1.0 s`, `256 MB`, `41 ms`). ID·해시·suite명에는 고정폭 글꼴을 사용합니다. 긴 값은 `truncate`로 줄여 표시하고 전체 값은 `title`로 제공합니다.
5. **오프라인**: 시스템 폰트 스택만 사용하며 외부 리소스에 의존하지 않습니다. CSP는 `default-src 'self'`로 설정합니다.

## 테마

- 기본 테마는 시스템 설정(`prefers-color-scheme`)을 따릅니다. 테마를 직접 선택하면 `<html data-theme="light|dark">`로 적용하고 `localStorage('aj.theme')`에 저장합니다. 시스템 설정을 선택하면 저장된 키를 삭제합니다.
- 첫 페인트 전에 `<head>`의 인라인 부트스트랩이 저장값을 읽어 `data-theme`을 적용해 깜박임을 방지합니다.
- `<aj-theme-switch>`는 시스템 → 라이트 → 다크 순환 버튼이며 현재 모드와 실제 적용 결과를 툴팁에 보여 줍니다.
- `color-scheme`을 함께 선언해 폼 컨트롤·스크롤바가 테마를 따릅니다.
- 팔레트 요약:

| 역할 | 라이트 | 다크 |
|---|---|---|
| 배경 / 보조 배경 / 인셋 | #ffffff / #f6f7f8 / #f9fafb | #161819 / #1c1e20 / #131415 |
| 경계선 / 강한 경계선 | #dfe2e6 / #c5c9cf | #2c3035 / #3d424a |
| 글자 / 보조 / 흐림 | #1b1d20 / #5c6370 / #8a919c | #d9dce0 / #9aa1ab / #6b727c |
| accent | #2463eb | #6091ff |
| ok / warn / err | #1a7f37 / #9a6700 / #c62828 | #4cc26b / #e0b341 / #f06a6a |
| system(채점기 오류·validator) | #6f42c1 | #b48cf0 |

다크 테마의 상태 배경은 해당 상태 색에 12% 알파를 적용하여 기본 배경색과 섞습니다. 두 테마 모두 본문과 보조 텍스트(`--c-fg-muted`)의 대비를 WCAG AA 기준인 4.5:1 이상으로 유지합니다. 흐림 텍스트(`--c-fg-faint`)는 캡션에만 사용하며 대비를 3:1 이상으로 유지합니다.

## 타이포그래피

| 용도 | 크기 | 행간 |
|---|---|---|
| UI 기본 | 13px | 1.45 |
| 보조·표 | 12px | 1.45 |
| 캡션·배지 | 11px | 1 |
| 문제 본문 | 14px | 1.6 |
| 페이지 제목 | 16px | 1.3 |
| 문제명 | 20px | 1.3 |

한글은 시스템 폰트(Apple SD Gothic Neo/Pretendard/Noto Sans KR)로 대체됩니다. 코드와 ID에는 `--font-mono`를 적용합니다. 문제 본문의 `h2`에는 영문 대문자 변환, 흐린 글자색, 하단의 가는 경계선을 적용하여 섹션(문제/입력/출력/노트/예제)을 구분합니다. 수식은 서버가 반환한 MathML을 그대로 렌더링하며 `display="block"`은 가로 스크롤을 허용합니다. 지원되지 않는 LaTeX 원문은 `.latex-raw`(mono, warn 색)로 표시합니다.

## 컴포넌트

UI는 Web Component(Light DOM, `aj-` 접두사)로 모듈화했습니다. 단순 값은 속성으로 받고, 목록·객체는 자식 `<script type="application/json">` 또는 `el.data` 프로퍼티로 받으며 API 응답 형태(`model.rs`)를 그대로 넣을 수 있게 설계했습니다. 컴포넌트가 DOM에 연결되거나 속성이 변경되면 다시 렌더링합니다. 상호작용 결과는 DOM 이벤트로 전달합니다.

| 컴포넌트 | 입력 | 출력/동작 | 데이터 출처 |
|---|---|---|---|
| `aj-topbar` | `crumbs`, `badges`(JSON) | 브랜드·경로·배지·테마 스위치 | capabilities |
| `aj-theme-switch` | — | 시스템→라이트→다크 순환, localStorage | — |
| `aj-tabs` | 자식 `role=tab` + `aria-controls` | `?tab=` 동기화, 방향키, `tab-change` | — |
| `aj-subnav` | JSON `[{id,label,count,badge}]`, `current` | 설정 9섹션 앵커 | manifest 집계 |
| `aj-segment` | `name`, `value`, 자식 `button[data-value]` | `aria-pressed`, `[data-panel-for=name]` 토글, `change` | — |
| `aj-verdict` | `code`, `size`, `label` | 글리프+약어+title | verdict 문자열 |
| `aj-trust` | `valid/invalid/uncertain/errors/historical` 또는 `data` | 현재 fingerprint 개수. 모두 0이면 “미검증” | trust_get |
| `aj-status` | `kind`, `title`, `detail`, 자식 버튼 | 준비 상태·결과 줄 (`role=status/alert`) | ready·regeneration |
| `aj-banner` | 본문, `message`, `note` | 오류 + 다시 시도 → `retry` | HTTP error |
| `aj-pane` | `title`, `copy`, `ws`, `tall`, `verdict`, 본문 | 공백 표시·복사, `value` | test_get·cases |
| `aj-example` | `number/input/output` 또는 JSON | 입력/출력 2열 aj-pane | problem_get.examples |
| `aj-ws-toggle` | `for` 셀렉터 | 여러 aj-pane 공백 일괄 토글 | — |
| `aj-opinion` | `status/provider/requested-model/response-model/at/rev/stale/direction` 또는 `data`(Review) | 의견 카드. 모델 표시 규칙(UI-SPEC) 내장 | reviews_list·trust_get |
| `aj-editor` | `language/filename/dirty/readonly/min-height`, 본문 | `value`, `bytes`, `change`. Monaco가 `.editor-body`를 대체 | — |
| `aj-dropzone` | `accept`, `max-bytes` | File API로 UTF-8 읽기 → `file` {name,size,text,ext} | — |
| `aj-argv` | `data`(string[]) | 칩 추가/제거 → `change`(string[]) | generate.args |
| `aj-submissions` | JSON Submission[], `test-href`, `current-generation`, `open` | 펼침 표, CE 진단, 통과 접기, `view-code` | submissions_list |
| `aj-tests` | JSON TestMeta[]+preview/trust, `test-href` | suite 테스트 표 | problem_get.tests |
| `aj-answers` | JSON Answer[]+trust, `main`, `validator`, `answer-href` | 답안 표, 역할 배지 | answers_list |
| `aj-test-runs` | JSON `[{answer_id,revision,expected,actual,passed,total,note}]` | expected/실제 일치 | test_run |
| `aj-validator-runs` | JSON validator_run[] | 기대·결과·일치 표 | validator_runs_list |
| `aj-revisions` | JSON `[{revision,created_at,trust}]`, `current` | 리비전 목록 | answer_history |
| `aj-diff` | 본문(unified diff) | +/−/@@ 색 + 접두 유지 | 클라이언트 diff |
| `aj-kv` | JSON `[[k, v]]` (v: 문자열/`{verdict}`/`{mono}`/`{faint}`/`{html}`) | 키/값 표 | — |
| `aj-problem-head` | JSON `{manifest}` | 이름·ID·제한·체커·suite·태그 | problem_get |
| `aj-problem-list` | JSON `[{manifest,ready,reason,test_count,submission_count}]`, `href`, `page-size`(기본 500), `sync-url` | 검색(이름·ID·태그)·준비 상태·태그 필터, 열 정렬(`aria-sort`, 이름/태그는 오름차순부터, 숫자는 내림차순부터), 500개 단위 페이지. 상태를 `?q=&ready=&tag=&sort=&dir=&page=`에 동기화하고 필터·정렬 변경 시 1페이지로 | problems_list |
| `aj-pager` | `total`, `page`, `page-size` | 범위 표시 + « ‹ 번호(현재±2, …) › » + 이동 입력 → `page-change` | — |

아래 표는 CSS 클래스와 태그로 구성하는 기본 UI 요소를 정리한 것입니다.

| 컴포넌트 | 클래스/태그 | 규칙 |
|---|---|---|
| 버튼 | `.btn`, `.primary`, `.ghost`, `.danger`, `.sm`, `.busy` | 26px(sm 22px). primary는 한 화면에 하나. 처리 중은 `.busy` + `disabled`. |
| 세그먼트 | `.btn-group` + `aria-pressed` | 에디터/파일 첨부 전환 등 상호배타 선택 |
| 입력 | `.input`, `.select`, `.textarea`, `.field` | 라벨 11px 위, 힌트/오류 아래. 숫자는 `.mono.num`. |
| 탭 | `<aj-tabs>` + `role=tab` | 하단 2px 언더라인, 방향키 이동, `?tab=` URL 동기화 |
| 서브내비 | `.subnav` | 채점 설정 9개 섹션. 168px 고정 폭, 720px 이하에서 가로 스크롤 |
| 배지 | `.badge` (`ok/warn/err/info/system/plain`) | 18px, 11px semibold. 텍스트 앞에 글리프(✓ ⚠ ✗) |
| 태그 | `.tag` | pill, 무채색 |
| Verdict | `<aj-verdict code="">` | AC ✓ / WA ✗ / PE ≈ / RE ! / TLE ⏱ / MLE ▣ / CE ⚙ / CHECKER_ERROR ⚠(system색). `size="lg"` |
| 상태 줄 | `.status.ok/warn/err/info` | 왼쪽 3px 색 띠. 제목 + 원인 + 오른쪽 조치 버튼. 준비 상태 표시에 사용 |
| 표 | `.table`, `.compact`, `.expander`, `.detail` | sticky 헤더, 행 28px(compact 24px). 펼침은 첫 열 22px 버튼 + `aria-expanded` + 다음 `.detail` 행 |
| 리스트 | `.list-item` | 표 대신 노션식 행이 필요할 때 |
| 패널 | `.pane`, `.pane-head`, `.io-grid(.three)` | 입력/기대 출력/제출 출력. 헤더 26px에 크기·복사·공백 토글. 최대 높이 320px(tall 60vh) 후 스크롤 |
| 공백 표시 | `[data-ws-toggle]` → `pre.ws` | 공백 `·`, 탭 `→`, 개행 `↵`를 `--c-ws` 색으로 |
| 에디터 | `.editor` | Monaco 마운트 영역. 테두리 strong, focus-within 링. 하단 22px 상태 줄(언어·인코딩·미저장·커서). 토큰 색은 `--tok-*`에서 Monaco 테마로 매핑 |
| 드롭존 | `.dropzone`, `.file-row` | 파일 첨부. 활성 시 accent 점선 |
| 카드 | `.card`, `.card-head`, `.card-body` | 설정 섹션 컨테이너. 헤더 30px에 제목 + 우측 동작 |
| 키/값 | `.kv` | 실행 결과, validator 근거 |
| 신뢰도 | `.trust` + `.n.valid/invalid/uncertain/error/none` | 현재 fingerprint의 개수만 표시. 퍼센트 없음. 미검증은 `.none` 텍스트 |
| 의견 카드 | `.opinion`, `.dir`, `.stale` | status 배지 + 프로바이더 + 리비전 + 시각. `.dir`은 수정 방향(왼쪽 2px 선). 과거 리비전은 `.stale`(점선, 70%) |
| 진단 | `.diag` | 컴파일 오류·시스템 오류. 왼쪽 2px err 선, pre-wrap |
| 상태 | `.empty`, `.skeleton`, `.inline-error` | 로딩/빈/오류 |
| 메뉴 | `.menu` | 유일하게 그림자를 쓰는 팝오버 |
| argv 입력 | `.argv` + `.chip` | generator 인수. 항목마다 칩 하나, Enter로 추가, × 로 제거. shell 문자열로 합치지 않습니다 |
| 옵션 카드 | `.options` + `.option` | checker 종류처럼 상호배타 선택이 3개 이상일 때. 라디오 + 제목 + 설명 |
| 분할 편집 | `.split` + `.editor` + `.preview` | Statements Markdown 편집·수식 미리보기. 900px 이하 1열 |
| 리비전 목록 | `.revs` + `.rev` | rN · 시각 · 해당 리비전 AI 요약. 현재는 `aria-current` |
| diff | `.diff` + `.hunk/.add/.del/.ctx` | 리비전 간 unified diff. 색 + 접두 기호(+/−) |
| 오류 배너 | `.banner` | 요청 실패 + 서버 진단(mono) + 다시 시도. 키·인증 헤더는 표시하지 않습니다 |

## 페이지 레이아웃

**셸**: 높이 36px의 상단바에 브랜드와 경로를 배치하고, 오른쪽에는 실행 모드·AI 연결 배지와 테마 스위치를 배치합니다. 본문은 최대 너비 1280px로 가운데 정렬하고 패딩을 12px로 설정합니다.

**/problems** (`ui/pages/index.html`): 제목과 새 문제 버튼을 배치하고, 그 아래에 `aj-problem-list`를 배치합니다. 필터 바(검색, 준비 상태, 태그, 필터 지우기, 개수, 페이저)는 상단바 아래에 sticky로 고정됩니다. 표에는 이름과 ID, 태그, 시간, 메모리, 테스트 수, 채점 준비 상태, 제출 수를 표시합니다. 태그를 클릭하면 해당 태그로 필터링하고, 열 헤더를 클릭하면 해당 열을 기준으로 정렬합니다. 500개 단위로 페이지를 나누고 표가 한 페이지를 넘기면 하단에도 페이저를 둡니다. problems_list는 전체를 반환하므로 정렬·필터링·페이지 나누기는 클라이언트에서 처리합니다. 준비 상태는 원인과 함께 하나의 배지로 요약합니다(`⚠ 답안 만료 · 정해 변경`).

**/problems/[id]** (`problem.html`): 페이지 헤더의 왼쪽에는 이름·ID·메타데이터(제한/체커/suite/태그)를, 오른쪽에는 준비 상태 줄과 조치 버튼을 배치합니다. 그 아래에는 다음 네 탭을 배치합니다.

- 문제: `.prose` 단일 컬럼 78ch. 예제는 입력/출력 2열 패널.
- 제출: 툴바(에디터/파일 세그먼트 · 언어 · 파일명 · 크기/한도 · 제출). 에디터 360px 이상. 파일 모드는 드롭존 + 파일 행. 전송 대상은 하나뿐임을 세그먼트로 강제.
- 채점 결과: 제출 표(펼침, 시각, ID 8자, 언어, verdict, 통과, 최대 시간·메모리, 코드 버튼). 펼치면 generation 비교·실패 수·신뢰도 요약과 compact 테스트 표(suite, #링크, verdict, 시간, 메모리, checker 메시지, AI 검증, 상세 링크). CE는 진단 블록만. 통과 테스트는 접어 두고 “모두 펼치기”.
- 채점 설정: 좌측 서브내비(9섹션, 개수 배지) + 우측 카드 스택. Statements는 섹션 세그먼트 + 분할 편집/미리보기 + 예시 쌍. Validator는 answer 선택·연결, 유효/무효 기대 라디오와 실행 기록(기대·결과·일치). Generator는 answer·seed·대상 suite/순번·argv 칩. Checker는 옵션 카드(tokens/exact/lines/testlib/custom)와 종료 코드 설명. 각 카드 헤더에 섹션 번호·제목·동작. Answers는 UUID 8자·rev·언어·expected verdict·역할 배지(정해/validator)·현재 리비전 AI 요약. Tests는 suite 셀렉트 + 수동/생성/전체 재생성. Test runs는 expected vs 실제 verdict와 일치 배지. AI는 fingerprint·요약·의견 카드(과거 리비전은 stale).

**/problems/[id]/answers/[uuid]** (`answer.html`): 헤더에 전체 UUID, 역할 배지, rev, 언어, expected를 표시하고 재생성 결과를 상태 줄로 제공합니다. 왼쪽에는 너비 168px의 리비전 목록을, 오른쪽에는 diff와 전체 코드를 전환하는 카드를 배치합니다. 현재 리비전에 대한 AI 의견에는 오류 상태를 포함하며, 과거 리비전 이력은 stale로 표시합니다.

**/problems/[id]/tests/[suite]/[n]** (`test.html`): 헤더에 출처·generation·해시를 표시하고, 오른쪽에는 공백 표시·다운로드·설정 이동 버튼을 배치합니다. 상태 배지 줄에는 생성 상태, validator 결과, 신뢰도, 스냅샷 여부를 표시합니다. 입력·기대 출력·제출 출력은 3열 패널로 배치하고 차이는 `.diff .del/.add`로 표시합니다. 실행 결과(kv)와 validator 근거는 2열 카드로 배치하며, AI 검증 카드도 제공합니다.

## 상호작용·접근성

- 모든 펼침·탭·세그먼트는 버튼이며 키보드로 조작합니다. 탭은 방향키와 Home/End로 이동하며, 상세 내용은 Enter/Space로 펼칩니다.
- 포커스 링은 두께 2px의 강조색으로 표시하고, 입력 요소 내부에는 -1px 오프셋을 적용합니다.
- 처리 중인 버튼에는 스피너와 disabled를 적용하고, 결과가 도착하면 해당 제출 행을 펼치고 `aria-selected`로 강조합니다.
- 표 헤더는 sticky로 고정합니다. 텍스트가 패널의 최대 높이를 넘으면 스크롤하거나 “더 보기”로 확인할 수 있도록 합니다.
- `prefers-reduced-motion`이 적용되면 전환 시간을 0으로 설정합니다.
- 화면 너비가 720px 이하이면 페이지 헤더를 세로로 배치하고, 설정 서브내비에 가로 스크롤을 적용하며, 3열 패널을 1열로 전환합니다.

## 구현 인계 메모

- 목업의 `src/*.js`는 TypeScript로 옮기되 DOM 구조·클래스명·ARIA는 유지합니다.
- Monaco 테마는 `--c-bg-inset`, `--c-fg`, `--tok-*`를 읽어 `defineTheme`으로 라이트·다크 테마를 생성합니다. `data-theme`이나 미디어 쿼리가 변경되면 `setTheme`으로 해당 테마를 적용합니다.
- 서버 응답 필드 매핑: `ready` → 상태 줄, `verdict` → `<aj-verdict>`, `trust_get`의 valid/invalid/uncertain/errors → `.trust`, `historical_count` → “이력 N”, `memory_kb=0` → “측정 불가”.
