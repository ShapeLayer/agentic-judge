import { Base, define, esc, attr, readData } from '../util.js';
import '../theme.js';
// <aj-topbar crumbs='[{"label":"문제","href":"index.html"},{"label":"A+B"}]' badges='[{"label":"컨테이너 실행","title":"…"}]'>
class Topbar extends Base {
  static observedAttributes = ['crumbs', 'badges', 'home'];
  render() {
    const crumbs = JSON.parse(attr(this, 'crumbs', '[]')); const badges = JSON.parse(attr(this, 'badges', '[]'));
    this.innerHTML = `<header class="topbar"><a class="brand" href="${esc(attr(this, 'home', 'index.html'))}"><span class="mark" aria-hidden="true">AJ</span>Agentic Judge</a>
      <nav class="crumbs" aria-label="경로">${crumbs.map((c, i) => `<span class="sep">/</span>${i === crumbs.length - 1 ? `<span class="current${c.mono ? ' mono' : ''}">${esc(c.label)}</span>` : `<a href="${esc(c.href ?? '#')}">${esc(c.label)}</a>`}`).join('')}</nav>
      <span class="grow"></span>${badges.map(b => `<span class="badge plain small" title="${esc(b.title ?? '')}">${esc(b.label)}</span>`).join('')}<aj-theme-switch></aj-theme-switch></header>`;
  }
}
define('aj-topbar', Topbar);

// <aj-tabs> wraps role=tablist buttons with data-tab + aria-controls; syncs ?tab=.
class Tabs extends HTMLElement {
  connectedCallback() {
    this.tabs = [...this.querySelectorAll('[role="tab"]')];
    this.panels = this.tabs.map(t => document.getElementById(t.getAttribute('aria-controls')));
    const param = new URL(location.href).searchParams.get('tab');
    this.select(this.tabs.find(t => t.dataset.tab === param) || this.tabs.find(t => t.getAttribute('aria-selected') === 'true') || this.tabs[0], false);
    this.addEventListener('click', e => { const t = e.target.closest('[role="tab"]'); if (t) this.select(t); });
    this.addEventListener('keydown', e => {
      const i = this.tabs.indexOf(document.activeElement); if (i < 0) return;
      const d = { ArrowRight: 1, ArrowLeft: -1, Home: -i, End: this.tabs.length - 1 - i }[e.key]; if (d === undefined) return;
      e.preventDefault(); const n = this.tabs[(i + d + this.tabs.length) % this.tabs.length]; n.focus(); this.select(n);
    });
  }
  select(tab, pushUrl = true) {
    this.tabs.forEach((t, i) => { const on = t === tab; t.setAttribute('aria-selected', on); t.tabIndex = on ? 0 : -1; if (this.panels[i]) this.panels[i].hidden = !on; });
    if (pushUrl) { const u = new URL(location.href); u.searchParams.set('tab', tab.dataset.tab); history.replaceState(null, '', u); }
    this.dispatchEvent(new CustomEvent('tab-change', { bubbles: true, detail: tab.dataset.tab }));
  }
}
define('aj-tabs', Tabs);

// <aj-subnav current="basic"><script type="application/json">[{"id":"basic","label":"기본 정보","count":4,"badge":"warn"}]</script></aj-subnav>
class Subnav extends Base {
  static observedAttributes = ['current'];
  render() {
    const items = readData(this, []) ?? []; const cur = attr(this, 'current', items[0]?.id);
    this.innerHTML = `<nav class="subnav" aria-label="채점 설정">${items.map((it, i) => `<a href="#s-${esc(it.id)}"${it.id === cur ? ' aria-current="page"' : ''}><span><span class="num">${i + 1}</span>${esc(it.label)}</span>${it.count != null ? `<span class="badge ${esc(it.badge ?? 'plain')} xs">${esc(it.count)}</span>` : ''}</a>`).join('')}</nav>`;
    this.querySelectorAll('a').forEach(a => a.addEventListener('click', () => this.setAttribute('current', a.getAttribute('href').slice(3))));
  }
}
define('aj-subnav', Subnav);

// <aj-segment name="mode" value="editor"><button data-value="editor">에디터</button>…</aj-segment> → 'change' {detail:value}; toggles [data-panel-for="name"] panels
class Segment extends Base {
  static observedAttributes = ['value'];
  render() {
    if (!this._btns) { this._btns = [...this.querySelectorAll('button')]; this.innerHTML = ''; const g = document.createElement('div'); g.className = 'btn-group'; g.setAttribute('role', 'group'); g.setAttribute('aria-label', attr(this, 'label', '')); g.append(...this._btns); this.append(g); this._btns.forEach(b => { b.classList.add('btn'); if (this.hasAttribute('small')) b.classList.add('sm'); b.type = 'button'; b.onclick = () => this.setAttribute('value', b.dataset.value); }); }
    const v = attr(this, 'value', this._btns[0]?.dataset.value);
    this._btns.forEach(b => b.setAttribute('aria-pressed', b.dataset.value === v));
    const name = attr(this, 'name'); if (name) document.querySelectorAll(`[data-panel-for="${name}"]`).forEach(p => p.hidden = p.dataset.panel !== v);
    this.dispatchEvent(new CustomEvent('change', { bubbles: true, detail: v }));
  }
  get value() { return attr(this, 'value'); }
}
define('aj-segment', Segment);
