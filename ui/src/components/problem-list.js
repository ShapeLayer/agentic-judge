import { Base, define, esc, attr, readData, fmtMs } from '../util.js';

// <aj-pager total="1234" page="1" page-size="500"> → 'page-change' {detail: page}
// Compact page control: 처음/이전/번호(현재±2, 생략 …)/다음/마지막 + 범위 표시.
class Pager extends Base {
  static observedAttributes = ['total', 'page', 'page-size'];
  render() {
    const total = +attr(this, 'total', 0), size = Math.max(1, +attr(this, 'page-size', 500));
    const pages = Math.max(1, Math.ceil(total / size)); const page = Math.min(pages, Math.max(1, +attr(this, 'page', 1)));
    const from = total ? (page - 1) * size + 1 : 0, to = Math.min(total, page * size);
    const nums = []; for (let p = 1; p <= pages; p++) if (p === 1 || p === pages || Math.abs(p - page) <= 2) nums.push(p);
    const items = []; for (let i = 0; i < nums.length; i++) { if (i && nums[i] - nums[i - 1] > 1) items.push('…'); items.push(nums[i]); }
    const b = (label, p, extra = '') => `<button class="btn ghost sm${p === page && typeof label === 'number' ? ' current' : ''}" type="button" data-page="${p}" ${p < 1 || p > pages || (p === page && typeof label !== 'number') ? 'disabled' : ''}${extra}>${label}</button>`;
    this.innerHTML = `<nav class="pager" aria-label="페이지"><span class="range num">${total ? `${from.toLocaleString()}–${to.toLocaleString()}` : '0'} / ${total.toLocaleString()}</span>
      ${pages > 1 ? `${b('«', 1, ' aria-label="첫 페이지"')}${b('‹', page - 1, ' aria-label="이전 페이지"')}${items.map(x => x === '…' ? '<span class="ellipsis" aria-hidden="true">…</span>' : b(x, x, x === page ? ' aria-current="page"' : ` aria-label="${x} 페이지"`)).join('')}${b('›', page + 1, ' aria-label="다음 페이지"')}${b('»', pages, ' aria-label="마지막 페이지"')}
      <label class="jump xs muted">이동 <input class="input mono num" type="number" min="1" max="${pages}" value="${page}" aria-label="페이지 번호"></label>` : ''}</nav>`;
    this.querySelectorAll('[data-page]').forEach(x => x.onclick = () => this.go(+x.dataset.page));
    const jump = this.querySelector('.jump input'); if (jump) jump.onchange = () => this.go(+jump.value);
  }
  go(p) { const pages = Math.max(1, Math.ceil(+attr(this, 'total', 0) / +attr(this, 'page-size', 500))); p = Math.min(pages, Math.max(1, p || 1)); if (p === +attr(this, 'page', 1)) return; this.setAttribute('page', p); this.dispatchEvent(new CustomEvent('page-change', { bubbles: true, detail: p })); }
}
define('aj-pager', Pager);

// <aj-problem-list href="problem.html?id={id}" page-size="500" sync-url>
//   <script type="application/json">[{manifest, ready, reason, test_count, submission_count}]</script>
// 클라이언트 측 검색(이름·ID·태그)·준비 상태·태그 필터, 열 정렬(aria-sort), 500개 단위 페이지.
// 상태는 ?q=&ready=&tag=&sort=&dir=&page= 로 동기화하며(sync-url) 필터·정렬 변경 시 1페이지로 돌아간다.
const COLS = [
  ['name', '이름', '', p => p.manifest.name],
  ['tags', '태그', '', p => (p.manifest.tags ?? []).join(',')],
  ['time', '시간', 'r', p => p.manifest.time_limit_ms],
  ['memory', '메모리', 'r', p => p.manifest.memory_limit_mb],
  ['tests', '테스트', 'r', p => p.test_count ?? 0],
  ['ready', '채점 준비', '', p => p.ready ? 0 : p.reason?.kind === 'warn' ? 1 : p.reason?.kind === 'err' ? 2 : 3],
  ['subs', '제출', 'r', p => p.submission_count ?? 0],
];
const readyBadge = p => p.ready ? `<span class="badge ok">✓ 채점 가능</span>` : p.reason?.kind === 'err' ? `<span class="badge err">✗ ${esc(p.reason.text)}</span>` : p.reason?.kind === 'warn' ? `<span class="badge warn">⚠ ${esc(p.reason.text)}</span>` : `<span class="badge">— ${esc(p.reason?.text ?? '준비 안 됨')}</span>`;
const collator = new Intl.Collator('ko', { numeric: true, sensitivity: 'base' });

class ProblemList extends Base {
  static observedAttributes = ['page-size'];
  connectedCallback() {
    if (!this.state) {
      const u = new URL(location.href).searchParams; const sync = this.hasAttribute('sync-url');
      this.state = { q: sync ? u.get('q') ?? '' : '', ready: sync ? u.get('ready') ?? 'all' : 'all', tag: sync ? u.get('tag') ?? '' : '', sort: sync ? u.get('sort') ?? 'name' : 'name', dir: sync ? u.get('dir') ?? 'asc' : 'asc', page: sync ? +(u.get('page') ?? 1) : 1 };
    }
    super.connectedCallback();
  }
  update(patch, resetPage = true) {
    Object.assign(this.state, patch, resetPage ? { page: 1 } : {});
    if (this.hasAttribute('sync-url')) { const u = new URL(location.href); for (const [k, v] of Object.entries(this.state)) { const def = { q: '', ready: 'all', tag: '', sort: 'name', dir: 'asc', page: 1 }[k]; v === def ? u.searchParams.delete(k) : u.searchParams.set(k, v); } history.replaceState(null, '', u); }
    this.render();
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, detail: { ...this.state } }));
  }
  filtered() {
    const all = readData(this, []) ?? []; const { q, ready, tag, sort, dir } = this.state; const needle = q.trim().toLowerCase();
    let rows = all.filter(p => {
      const m = p.manifest;
      if (needle && !(m.name.toLowerCase().includes(needle) || m.id.toLowerCase().includes(needle) || (m.tags ?? []).some(t => t.toLowerCase().includes(needle)))) return false;
      if (ready === 'ready' && !p.ready) return false; if (ready === 'not' && p.ready) return false;
      if (tag && !(m.tags ?? []).includes(tag)) return false;
      return true;
    });
    const col = COLS.find(c => c[0] === sort) ?? COLS[0]; const key = col[3]; const sign = dir === 'desc' ? -1 : 1;
    rows.sort((a, b) => { const x = key(a), y = key(b); const c = typeof x === 'number' ? x - y : collator.compare(x, y); return (c || collator.compare(a.manifest.name, b.manifest.name)) * sign; });
    return { all, rows };
  }
  render() {
    const { all, rows } = this.filtered(); const s = this.state; const size = Math.max(1, +attr(this, 'page-size', 500));
    const pages = Math.max(1, Math.ceil(rows.length / size)); s.page = Math.min(pages, Math.max(1, s.page));
    const slice = rows.slice((s.page - 1) * size, s.page * size); const href = attr(this, 'href', '#');
    const tags = [...new Set(all.flatMap(p => p.manifest.tags ?? []))].sort(collator.compare);
    const th = ([k, label, cls]) => `<th class="${cls} sortable" aria-sort="${s.sort === k ? (s.dir === 'asc' ? 'ascending' : 'descending') : 'none'}"><button type="button" class="sort" data-sort="${k}">${label}<span class="arrow" aria-hidden="true">${s.sort === k ? (s.dir === 'asc' ? '▲' : '▼') : ''}</span></button></th>`;
    const row = p => { const m = p.manifest; return `<tr><td><a href="${esc(href.replace('{id}', encodeURIComponent(m.id)))}" class="strong">${esc(m.name)}</a> <span class="id mono faint">${esc(m.id)}</span></td><td><span class="tags">${(m.tags ?? []).map(t => `<button type="button" class="tag tag-btn" data-tag="${esc(t)}" title="태그로 필터">${esc(t)}</button>`).join('')}</span></td><td class="r num">${fmtMs(m.time_limit_ms)}</td><td class="r num">${m.memory_limit_mb} MB</td><td class="r num">${p.test_count ?? 0}</td><td>${readyBadge(p)}</td><td class="r num">${p.submission_count ?? 0}</td></tr>`; };
    const filtering = s.q || s.ready !== 'all' || s.tag;
    this.innerHTML = `<div class="toolbar filters">
        <input class="input" type="search" placeholder="이름·ID·태그 검색" aria-label="문제 검색" value="${esc(s.q)}" style="width:220px">
        <select class="select" aria-label="준비 상태 필터"><option value="all"${s.ready === 'all' ? ' selected' : ''}>모든 상태</option><option value="ready"${s.ready === 'ready' ? ' selected' : ''}>채점 가능</option><option value="not"${s.ready === 'not' ? ' selected' : ''}>준비 안 됨</option></select>
        <select class="select" aria-label="태그 필터"><option value="">모든 태그</option>${tags.map(t => `<option value="${esc(t)}"${s.tag === t ? ' selected' : ''}>${esc(t)}</option>`).join('')}</select>
        ${filtering ? `<button class="btn ghost sm" type="button" data-clear>필터 지우기</button>` : ''}
        <span class="spacer"></span>
        <span class="muted small num" role="status">${filtering ? `${rows.length.toLocaleString()} / ${all.length.toLocaleString()}개` : `${all.length.toLocaleString()}개`}</span>
        <aj-pager total="${rows.length}" page="${s.page}" page-size="${size}"></aj-pager>
      </div>
      ${rows.length ? `<div class="table-wrap"><table class="table"><thead><tr>${COLS.map(th).join('')}</tr></thead><tbody>${slice.map(row).join('')}</tbody></table></div>` : all.length ? `<div class="empty"><span>조건에 맞는 문제가 없습니다.</span><button class="btn sm" type="button" data-clear>필터 지우기</button></div>` : `<div class="empty"><span>등록된 문제가 없습니다.</span><span class="xs">새 문제를 만들거나 MCP register_problem 프롬프트로 등록합니다.</span></div>`}
      ${rows.length > size ? `<div class="toolbar" style="justify-content:flex-end"><aj-pager total="${rows.length}" page="${s.page}" page-size="${size}"></aj-pager></div>` : ''}`;
    // wiring
    const input = this.querySelector('input[type=search]'); let t;
    input.oninput = () => { clearTimeout(t); t = setTimeout(() => { const pos = input.selectionStart; this.update({ q: input.value }); const i = this.querySelector('input[type=search]'); i.focus(); i.setSelectionRange(pos, pos); }, 150); };
    const [readySel, tagSel] = this.querySelectorAll('select'); readySel.onchange = () => this.update({ ready: readySel.value }); tagSel.onchange = () => this.update({ tag: tagSel.value });
    this.querySelectorAll('[data-clear]').forEach(b => b.onclick = () => this.update({ q: '', ready: 'all', tag: '' }));
    this.querySelectorAll('[data-sort]').forEach(b => b.onclick = () => { const k = b.dataset.sort; this.update(k === s.sort ? { dir: s.dir === 'asc' ? 'desc' : 'asc' } : { sort: k, dir: k === 'name' || k === 'tags' ? 'asc' : 'desc' }); this.querySelector(`[data-sort="${k}"]`)?.focus(); });
    this.querySelectorAll('[data-tag]').forEach(b => b.onclick = () => this.update({ tag: b.dataset.tag }));
    this.querySelectorAll('aj-pager').forEach(p => p.addEventListener('page-change', e => { e.stopPropagation(); this.update({ page: e.detail }, false); this.querySelector('.table-wrap')?.scrollIntoView({ block: 'start' }); }));
  }
}
define('aj-problem-list', ProblemList);
