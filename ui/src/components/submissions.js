import { Base, define, esc, attr, readData, fmtMs, fmtKb, fmtTime, short, fill } from '../util.js';
import { verdictHtml } from './verdict.js';
import { trustHtml } from './trust.js';
// <aj-submissions test-href="test.html?suite={suite}&n={number}" current-generation="…" open="uuid">
//   <script type="application/json">[Submission, …]</script>
// </aj-submissions>
// Submission = model.rs Submission (+ optional trust per case: case.trust, submission.trust).
// CE rows show a compile diagnostic, never a fake test #0. Passed cases are collapsed by default.
class Submissions extends Base {
  static observedAttributes = ['open', 'current-generation'];
  render() {
    const subs = readData(this, []) ?? [];
    const href = attr(this, 'test-href', '#');
    const gen = attr(this, 'current-generation');
    const open = attr(this, 'open');
    if (!subs.length) { this.innerHTML = `<div class="empty"><span>아직 제출이 없습니다.</span><span class="xs">제출 탭에서 코드를 보내면 여기에 결과가 쌓입니다.</span></div>`; return; }
    const row = s => {
      const cases = s.cases ?? [];
      const passed = cases.filter(c => c.result.verdict === 'AC').length;
      const isCE = s.verdict === 'CE';
      const maxT = Math.max(0, ...cases.map(c => c.result.time_ms)), maxM = Math.max(0, ...cases.map(c => c.result.memory_kb));
      const tle = cases.some(c => c.result.verdict === 'TLE');
      const id = `sub-${s.id}`; const isOpen = s.id === open;
      return `<tr${isOpen ? ' aria-selected="true"' : ''}>
        <td><button class="expander" type="button" aria-expanded="${isOpen}" aria-controls="${id}" aria-label="제출 ${short(s.id)} 상세 펼치기"></button></td>
        <td class="num">${fmtTime(s.created_at)}</td><td class="mono"><a href="#${id}" title="${esc(s.id)}">${short(s.id)}</a></td><td>${esc(s.language)}</td>
        <td>${verdictHtml(s.verdict)}</td>
        <td class="r num${isCE ? ' muted' : ''}">${isCE ? '—' : `${passed} / ${cases.length}`}</td>
        <td class="r num${isCE ? ' muted' : ''}">${isCE ? '—' : (tle ? '&gt; ' : '') + fmtMs(maxT)}</td>
        <td class="r num${isCE ? ' muted' : ''}">${isCE ? '—' : fmtKb(maxM)}</td>
        <td class="r"><button class="btn ghost sm" type="button" data-code="${esc(s.id)}">코드</button></td></tr>
        <tr class="detail" id="${id}"${isOpen ? '' : ' hidden'}><td colspan="9">${isCE ? ce(s) : detail(s, cases, passed)}</td></tr>`;
    };
    const ce = s => `<div class="diag">${esc(s.cases?.[0]?.result.stderr || s.compile_output || '')}</div><p class="xs faint" style="margin-top:var(--sp-2)">컴파일 진단입니다. 테스트 링크를 만들지 않습니다.</p>`;
    const detail = (s, cases, passed) => {
      const same = gen && s.generation === gen;
      const failed = cases.filter(c => c.result.verdict !== 'AC');
      const shown = failed.length ? failed : cases;
      const caseRow = c => `<tr><td class="mono">${esc(c.suite)}</td><td class="r num"><a href="${fill(href, c)}">${c.number}</a></td><td>${verdictHtml(c.result.verdict)}</td><td class="r num">${fmtMs(c.result.time_ms)}</td><td class="r num">${fmtKb(c.result.memory_kb)}</td><td class="wrap">${esc(c.checker_message)}${c.result.stderr ? `<div class="xs faint mono truncate" title="${esc(c.result.stderr)}">stderr: ${esc(c.result.stderr)}</div>` : ''}</td><td>${trustHtml(c.trust)}</td><td class="r"><a href="${fill(href, c)}" class="xs">상세</a></td></tr>`;
      return `<div class="row between" style="margin-bottom:var(--sp-3)"><span class="small muted">generation <span class="mono">${esc(short(s.generation))}</span> ${gen ? (same ? '(현재와 동일)' : '— 현재 테스트와 다른 과거 스냅샷') : ''} · 테스트 ${cases.length} · 실패 ${failed.length}</span>${trustHtml(s.trust)}</div>
        <div class="table-wrap"><table class="table compact"><thead><tr><th>Suite</th><th class="r">#</th><th>Verdict</th><th class="r">시간</th><th class="r">메모리</th><th>Checker</th><th>AI 검증</th><th></th></tr></thead><tbody>${shown.map(caseRow).join('')}
        ${failed.length && failed.length < cases.length ? `<tr class="subtle"><td colspan="8" class="c xs">통과한 나머지 ${passed}개 테스트 · <button class="btn ghost sm" type="button" data-all="${esc(s.id)}">모두 펼치기</button></td></tr>` : ''}</tbody></table></div>`;
    };
    this.innerHTML = `<div class="table-wrap"><table class="table"><thead><tr><th style="width:28px"></th><th>제출 시각</th><th>ID</th><th>언어</th><th>Verdict</th><th class="r">통과</th><th class="r">최대 시간</th><th class="r">최대 메모리</th><th></th></tr></thead><tbody>${subs.map(row).join('')}</tbody></table></div>`;
    this.addEventListener('click', e => {
      const b = e.target.closest('.expander'); if (b) { const on = b.getAttribute('aria-expanded') !== 'true'; b.setAttribute('aria-expanded', on); this.querySelector('#' + b.getAttribute('aria-controls')).hidden = !on; b.closest('tr').setAttribute('aria-selected', on); return; }
      const c = e.target.closest('[data-code]'); if (c) this.dispatchEvent(new CustomEvent('view-code', { bubbles: true, detail: subs.find(s => s.id === c.dataset.code) }));
      const a = e.target.closest('[data-all]'); if (a) { const s = subs.find(x => x.id === a.dataset.all); a.closest('tbody').innerHTML = s.cases.map(x => detailRowOf(x)).join(''); }
    }, { once: false });
    const detailRowOf = c => `<tr><td class="mono">${esc(c.suite)}</td><td class="r num"><a href="${fill(href, c)}">${c.number}</a></td><td>${verdictHtml(c.result.verdict)}</td><td class="r num">${fmtMs(c.result.time_ms)}</td><td class="r num">${fmtKb(c.result.memory_kb)}</td><td class="wrap">${esc(c.checker_message)}</td><td>${trustHtml(c.trust)}</td><td class="r"><a href="${fill(href, c)}" class="xs">상세</a></td></tr>`;
  }
}
define('aj-submissions', Submissions);
