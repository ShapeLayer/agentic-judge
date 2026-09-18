# ui/: 프론트엔드 디자인 시스템과 목업

빌드 없이 정적으로 열 수 있는 디자인 산출물입니다. 실제 구현(TypeScript, Monaco, API 연결)은 [docs/UI-SPEC.md](../docs/UI-SPEC.md)와 [docs/UI-DESIGN.md](../docs/UI-DESIGN.md)를 따릅니다.

```sh
python3 -m http.server 8765 --directory ui
# http://127.0.0.1:8765/pages/styleguide.html
```

- `styles/tokens.css`: 색·간격·타이포·레이아웃 토큰. 라이트/다크 팔레트와 테마 결정 규칙.
- `styles/base.css`: 리셋, 앱 셸(topbar/main), 문제 본문 prose.
- `styles/components.css`: 버튼·입력·탭·표·배지·verdict·패널·에디터 기본 구조·신뢰도·의견 카드.
- `src/index.js`: 페이지에서 로드하는 유일한 스크립트입니다. 아래 컴포넌트를 모두 등록합니다.
- `src/util.js`: `Base`(connect/attribute 변경 시 render, `data` 프로퍼티), `readData`(자식 `<script type="application/json">` 또는 `data` 속성), 포맷터, `esc`.
- `src/theme.js`: `<aj-theme-switch>`. 시스템 → 라이트 → 다크 순환, `localStorage('aj.theme')`.
- `src/components/`: Light DOM 컴포넌트. 단순 값은 속성으로 받고, 목록과 객체는 자식 JSON 또는 `el.data`로 받습니다. API 요약은 스타일 가이드와 [UI-DESIGN.md](../docs/UI-DESIGN.md)의 컴포넌트 표를 참고합니다.
  - `verdict.js` `<aj-verdict>` · `trust.js` `<aj-trust>` · `status.js` `<aj-status>` `<aj-banner>`
  - `pane.js` `<aj-pane>` `<aj-example>` `<aj-ws-toggle>` · `opinion.js` `<aj-opinion>`(모델 표시 규칙 포함)
  - `editor.js` `<aj-editor>`(Monaco 마운트 셸) · `argv.js` `<aj-argv>`
  - `submissions.js` `<aj-submissions>` · `tests.js` `<aj-tests>` `<aj-answers>`
  - `nav.js` `<aj-topbar>` `<aj-tabs>` `<aj-subnav>` `<aj-segment>`
  - `problem-list.js` `<aj-problem-list>`(정렬·필터·500개 페이지) `<aj-pager>`
  - `misc.js` `<aj-problem-head>` `<aj-dropzone>` `<aj-diff>` `<aj-revisions>` `<aj-kv>` `<aj-test-runs>` `<aj-validator-runs>`
- `pages/`: 문제 목록, 문제(4탭, 설정 9섹션), 테스트 상세, 답안 리비전/diff, 스타일 가이드 목업. 데이터는 목업에 직접 정의되어 있습니다.

컴포넌트는 Shadow DOM을 쓰지 않으므로 전역 토큰·클래스가 그대로 적용됩니다. 구현 단계에서 `.ts`로 옮기고 `data`에 API 응답을 그대로 넣습니다. 외부 CDN·폰트를 참조하지 않습니다.
