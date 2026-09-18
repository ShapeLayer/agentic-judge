import { Base, define, esc, attr, readData, fill, short } from '../util.js';
import { trustHtml } from './trust.js';
// <aj-tests test-href="test.html?suite={suite}&n={number}"><script type="application/json">[TestMeta + {preview, trust}]</script></aj-tests>
const genBadge = t => t.generation_error ? `<span class="badge err">실패 · ${esc(t.generation_error)}</span>` : t.generation ? `<span class="badge ok">생성됨</span>` : `<span class="badge">대기</span>`;
const valBadge = t => t.validation_status === 'accepted' || t.validation_status === 'ok' ? `<span class="badge ok">통과</span>` : t.validation_status ? `<span class="badge err">거부 · ${esc(t.validation_status)}</span>` : `<span class="badge">미검사</span>`;
const origin = t => t.origin?.generator_id ? `gen <span class="mono">${esc(short(t.origin.generator_id).slice(0, 4))}</span> r${esc(t.origin.revision)}${t.origin.args?.length ? ` [<span class="mono">${esc(t.origin.args.join(' '))}</span>]` : ''}` : '수동';
class Tests extends Base {
  render() {
    const tests = readData(this, []) ?? []; const href = attr(this, 'test-href', '#');
    if (!tests.length) { this.innerHTML = `<div class="empty"><span>이 suite에 테스트가 없습니다.</span><span class="xs">수동 입력 또는 생성기로 테스트를 추가합니다.</span></div>`; return; }
    this.innerHTML = `<table class="table compact"><thead><tr><th class="r">#</th><th>입력 미리보기</th><th>출처</th><th>생성</th><th>Validator</th><th>AI 검증</th><th></th></tr></thead><tbody>
      ${tests.map(t => `<tr><td class="r num"><a href="${fill(href, t)}">${t.number}</a></td><td class="mono truncate muted" style="max-width:220px" title="${esc(t.preview)}">${esc(t.preview)}</td><td class="xs">${origin(t)}</td><td>${genBadge(t)}</td><td>${valBadge(t)}</td><td>${trustHtml(t.trust)}</td><td class="r"><a class="xs" href="${fill(href, t)}">상세</a></td></tr>`).join('')}</tbody></table>`;
  }
}
define('aj-tests', Tests);

// <aj-answers answer-href="answer.html?id={id}" main="uuid" validator="uuid"><script type="application/json">[Answer + {trust, note}]</script></aj-answers>
import { verdictHtml } from './verdict.js';
import { fmtTime } from '../util.js';
class Answers extends Base {
  render() {
    const rows = readData(this, []) ?? []; const href = attr(this, 'answer-href', '#');
    const role = a => a.id === attr(this, 'main') ? `<span class="badge info">정해</span>` : a.id === attr(this, 'validator') ? `<span class="badge system">validator</span>` : a.role ? `<span class="badge">${esc(a.role)}</span>` : '<span class="muted">—</span>';
    if (!rows.length) { this.innerHTML = `<div class="empty"><span>저장된 답안이 없습니다.</span><span class="xs">정해를 추가하면 답안 생성이 시작됩니다.</span></div>`; return; }
    this.innerHTML = `<table class="table compact"><thead><tr><th>UUID</th><th class="r">rev</th><th>언어</th><th>expected</th><th>역할</th><th>AI (현재 리비전)</th><th class="r">수정</th><th></th></tr></thead><tbody>
      ${rows.map(a => `<tr><td class="mono"><a href="${fill(href, a)}" title="${esc(a.id)}">${short(a.id)}</a></td><td class="r num">${a.revision}</td><td>${esc(a.language)}</td><td>${verdictHtml(a.expected)}</td><td>${role(a)}</td><td>${trustHtml(a.trust)}${a.note ? ` <span class="xs faint">${esc(a.note)}</span>` : ''}</td><td class="r num muted">${fmtTime(a.created_at).slice(11)}</td><td class="r"><a class="btn ghost sm" href="${fill(href, a)}">이력</a></td></tr>`).join('')}</tbody></table>`;
  }
}
define('aj-answers', Answers);
