# 설치형 문제풀이 UI

`app/`의 HTML·CSS·TypeScript 앱을 Rust 바이너리에 내장합니다. `agentic-judge serve --open`으로 실행하며 같은 서버의 HTTP API를 사용합니다. 현재 동작은 [UI-SPEC.md](../docs/UI-SPEC.md), 화면 구성은 [UI-DESIGN.md](../docs/UI-DESIGN.md)에 문서화되어 있습니다.

| 파일 | 역할 |
|---|---|
| `app/index.html` | 문제 목록, 본문, 코드 작성·제출, 결과, 채점 준비 폼 |
| `app/app.ts` | API 호출, 문제 선택, 초안 저장·복구, 파일 열기·다운로드 |
| `app/types.ts` | 앱에서 사용하는 API 응답 필드의 TypeScript 타입 |
| `app/app.css` | 레이아웃·컨트롤·반응형 스타일 |
| `styles/tokens.css` | 색상·시스템 글꼴과 라이트/다크 토큰 |
| `../src/web_ui.rs` | HTML, 컴파일된 스크립트, CSS를 내장하고 URL에 매핑 |

## 개발과 빌드

Node.js 22 이상과 npm이 필요합니다.

```sh
npm ci --prefix ui
npm run check --prefix ui
npm run build --prefix ui
cargo run -- serve --open
```

`tsconfig.json`은 strict 검사 후 `dist/app/`에 ES 모듈을 생성합니다. `.ts`의 `.js` import 경로는 출력 모듈 기준입니다. `npm run watch --prefix ui`로 TypeScript를 자동 재컴파일할 수 있습니다. HTML과 정적 자원은 바이너리에 내장되므로 변경 사항을 서버에 반영하려면 Cargo를 다시 빌드하고 서버를 재시작해야 합니다. API가 필요하므로 정적 파일 서버만으로 앱을 실행할 수 없습니다.

최초 npm 의존성 설치 후 `cargo build`/`cargo test`는 `build.rs`에서 UI를 오프라인 컴파일합니다. `dist/`와 `node_modules/`는 버전 관리하지 않습니다. 배포 바이너리 사용자는 Node.js와 npm이 필요하지 않습니다.
