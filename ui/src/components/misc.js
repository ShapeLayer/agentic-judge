import { Base, define, esc, attr, readData, fmtBytes, fmtMs } from '../util.js';
import { verdictHtml } from './verdict.js';
import { trustHtml } from './trust.js';

// <aj-problem-head><script type="application/json">{manifest, test_count}</script></aj-problem-head>
class ProblemHead extends Base {
  render() {
    const d = readData(this, {}) ?? {}; const m = d.manifest ?? {};
    this.innerHTML = `<div class="col gap-2"><h1>${esc(m.name)} <span class="id">${esc(m.id)}</span></h1><div class="meta">
      <span><span class="k">시간 </span><span class="v num">${fmtMs(m.time_limit_ms ?? 0)}</span></span><span><span class="k">메모리 </span><span class="v num">${m.memory_limit_mb} MB</span></span>
      <span><span class="k">체커 </span><span class="v mono">${esc(m.checker)}</span></span><span><span class="k">suite </span><span class="v mono">${(m.suites ?? []).map(esc).join(' · ')}</span></span>
      ${m.tags?.length ? `<span class="tags">${m.tags.map(t => `<span class="tag">${esc(t)}</span>`).join('')}</span>` : ''}</div></div>`;
  }
}
define('aj-problem-head', ProblemHead);


// <aj-dropzone accept=".cpp,.py,…" max-bytes="4194304"> reads a UTF-8 file with the File API → 'file' {name,size,text,ext}
class Dropzone extends Base {
  render() {
    this.innerHTML = `<div class="dropzone" tabindex="0" role="button" aria-label="파일 선택 또는 드롭"><span>소스 파일을 여기에 놓거나 클릭하여 선택합니다.</span><span class="xs faint">UTF-8 · ${fmtBytes(+attr(this, 'max-bytes', 4194304))} 이하 · 확장자로 언어를 추정하며 언어 선택으로 덮어쓸 수 있습니다.</span><input type="file" hidden accept="${esc(attr(this, 'accept', ''))}"></div><div class="file-row" hidden></div>`;
    const zone = this.querySelector('.dropzone'), input = this.querySelector('input'), row = this.querySelector('.file-row');
    const pick = async file => {
      if (!file) return;
      if (file.size > +attr(this, 'max-bytes', 4194304)) { row.hidden = false; row.innerHTML = `<span class="inline-error">파일이 너무 큽니다: ${fmtBytes(file.size)}</span>`; return; }
      const text = await file.text(); const ext = file.name.split('.').pop();
      row.hidden = false; row.innerHTML = `<span class="mono">${esc(file.name)}</span><span class="muted num">${fmtBytes(file.size)}</span><span class="badge info">${esc(ext)}</span><span class="grow"></span><button class="btn ghost sm" type="button">제거</button>`;
      row.querySelector('button').onclick = () => { row.hidden = true; this._file = null; this.dispatchEvent(new CustomEvent('file', { bubbles: true, detail: null })); };
      this._file = { name: file.name, size: file.size, text, ext };
      this.dispatchEvent(new CustomEvent('file', { bubbles: true, detail: this._file }));
    };
    zone.onclick = () => input.click(); zone.onkeydown = e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); input.click(); } };
    input.onchange = () => pick(input.files[0]);
    zone.ondragover = e => { e.preventDefault(); zone.classList.add('active'); }; zone.ondragleave = () => zone.classList.remove('active');
    zone.ondrop = e => { e.preventDefault(); zone.classList.remove('active'); pick(e.dataTransfer.files[0]); };
  }
  get file() { return this._file ?? null; }
}
define('aj-dropzone', Dropzone);

// <aj-diff>unified diff text</aj-diff> — colors +/-/@@ lines; keeps the prefix characters so it reads without color.
class Diff extends Base {
  render() {
    if (this._text === undefined) this._text = this.data ?? this.textContent.replace(/^\n/, '');
    const cls = l => l.startsWith('@@') ? 'hunk' : l.startsWith('+') ? 'add' : l.startsWith('-') ? 'del' : 'ctx';
    this.innerHTML = `<div class="diff"><pre>${this._text.split('\n').map(l => `<span class="${cls(l)}">${esc(l) || ' '}</span>`).join('')}</pre></div>`;
  }
}
define('aj-diff', Diff);

// <aj-revisions current="3" href="answer.html?id={id}&rev={rev}"><script type="application/json">[{revision, created_at, trust}]</script></aj-revisions>
import { fmtTime } from '../util.js';
class Revisions extends Base {
  static observedAttributes = ['current'];
  render() {
    const revs = readData(this, []) ?? []; const cur = +attr(this, 'current', revs[0]?.revision);
    this.innerHTML = `<div class="revs" role="list">${revs.map(r => `<div class="rev" role="listitem"${r.revision === cur ? ' aria-current="true"' : ''}><span class="r">r${r.revision}</span><span class="muted num">${fmtTime(r.created_at).slice(11)}</span>${trustHtml(r.trust)}</div>`).join('')}</div>`;
  }
}
define('aj-revisions', Revisions);

// <aj-kv><script type="application/json">[["verdict", {"verdict":"WA"}], ["시간","13 ms"]]</script></aj-kv> — value may be string or {verdict}/{mono}/{html}
class Kv extends Base {
  render() {
    const rows = readData(this, []) ?? [];
    const v = x => typeof x === 'string' ? esc(x) : x?.verdict ? verdictHtml(x.verdict) : x?.mono ? `<span class="mono">${esc(x.mono)}</span>` : x?.faint ? `<span class="faint">${esc(x.faint)}</span>` : x?.html ?? '';
    this.innerHTML = `<dl class="kv">${rows.map(([k, val]) => `<dt>${esc(k)}</dt><dd>${v(val)}</dd>`).join('')}</dl>`;
  }
}
define('aj-kv', Kv);

// <aj-test-runs><script type="application/json">[{answer_id, revision, expected, actual, passed, total, note}]</script></aj-test-runs>
import { short } from '../util.js';
class TestRuns extends Base {
  render() {
    const rows = readData(this, []) ?? [];
    this.innerHTML = `<table class="table compact"><thead><tr><th>Answer</th><th>expected</th><th>실제</th><th>일치</th><th class="r">통과</th><th>비고</th></tr></thead><tbody>${rows.map(r => `<tr><td class="mono">${short(r.answer_id)} <span class="faint">r${r.revision}</span></td><td>${verdictHtml(r.expected)}</td><td>${r.actual ? verdictHtml(r.actual) : '<span class="muted">—</span>'}</td><td>${r.actual == null ? '<span class="badge">미실행</span>' : r.expected === r.actual ? '<span class="badge ok">✓ 일치</span>' : '<span class="badge err">✗ 불일치</span>'}</td><td class="r num">${r.passed ?? '—'}/${r.total ?? '—'}</td><td class="muted xs">${esc(r.note ?? '—')}</td></tr>`).join('')}</tbody></table>`;
  }
}
define('aj-test-runs', TestRuns);

// <aj-validator-runs><script type="application/json">[validator_run record]</script></aj-validator-runs>
class ValidatorRuns extends Base {
  render() {
    const rows = readData(this, []) ?? [];
    if (!rows.length) { this.innerHTML = `<div class="empty"><span>validator 실행 기록이 없습니다.</span></div>`; return; }
    this.innerHTML = `<table class="table compact"><thead><tr><th>시각</th><th>기대</th><th>결과</th><th>일치</th><th>입력</th><th>stderr</th></tr></thead><tbody>${rows.map(r => `<tr><td class="num">${fmtTime(r.created_at).slice(11)}</td><td>${r.expect_valid ? '승인' : '거부'}</td><td>${r.accepted ? '<span class="badge ok">승인</span>' : '<span class="badge err">거부</span>'}</td><td>${r.matched ? '<span class="badge ok">✓</span>' : '<span class="badge err">✗ 불일치</span>'}</td><td class="mono truncate" style="max-width:160px" title="${esc(r.input)}">${esc(r.input.trim())}</td><td class="mono xs muted truncate" style="max-width:240px" title="${esc(r.result?.stderr ?? '')}">${esc(r.result?.stderr || '—')}</td></tr>`).join('')}</tbody></table>`;
  }
}
define('aj-validator-runs', ValidatorRuns);
