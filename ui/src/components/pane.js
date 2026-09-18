import { Base, define, esc, attr, fmtBytes } from '../util.js';
// <aj-pane title="입력" copy ws tall>raw text</aj-pane>
// Whitespace toggle marks spaces/tabs/newlines; copy uses the raw text.
export const markWs = text => esc(text).replace(/ /g, '<span class="sp"> </span>').replace(/\t/g, '<span class="tab">\t</span>').replace(/\n/g, '<span class="nl"></span>\n');
class Pane extends Base {
  static observedAttributes = ['title', 'copy', 'ws', 'tall', 'verdict', 'size'];
  render() {
    if (this._raw === undefined) this._raw = this.data ?? this.textContent.replace(/^\n/, '');
    const size = this.hasAttribute('size') ? attr(this, 'size') : fmtBytes(new TextEncoder().encode(this._raw).length);
    this.innerHTML = `<div class="pane${this.hasAttribute('tall') ? ' tall' : ''}"><div class="pane-head"><span>${esc(attr(this, 'title'))}</span><span class="faint num">${esc(size)}</span>${this.hasAttribute('verdict') ? `<aj-verdict code="${esc(attr(this, 'verdict'))}"></aj-verdict>` : ''}<span class="spacer"></span>${this.hasAttribute('ws') ? `<button class="btn ghost sm" type="button" data-ws aria-pressed="false">공백</button>` : ''}${this.hasAttribute('copy') ? `<button class="btn ghost sm" type="button" data-copy>복사</button>` : ''}</div><pre></pre></div>`;
    this._pre = this.querySelector('pre'); this._pre.textContent = this._raw;
    this.querySelector('[data-ws]')?.addEventListener('click', e => this.toggleWs(e.currentTarget));
    this.querySelector('[data-copy]')?.addEventListener('click', async e => { try { await navigator.clipboard.writeText(this._raw); e.currentTarget.textContent = '복사됨'; setTimeout(() => e.currentTarget.textContent = '복사', 1200); } catch {} });
  }
  toggleWs(btn, force) {
    const on = force ?? btn?.getAttribute('aria-pressed') !== 'true';
    btn?.setAttribute('aria-pressed', on);
    this._pre.classList.toggle('ws', on);
    if (on) this._pre.innerHTML = markWs(this._raw); else this._pre.textContent = this._raw;
  }
  get value() { return this._raw; }
  set value(v) { this._raw = v; this.render(); }
}
define('aj-pane', Pane);

// <aj-example number="1" input="2 3\n" output="5\n"> → input/output pair with copy buttons
class Example extends Base {
  static observedAttributes = ['number', 'input', 'output'];
  render() {
    const d = this.data ?? { number: attr(this, 'number'), input: attr(this, 'input'), output: attr(this, 'output') };
    this.innerHTML = `<div class="example"><aj-pane title="예제 입력 ${esc(d.number)}" copy></aj-pane><aj-pane title="예제 출력 ${esc(d.number)}" copy></aj-pane></div>`;
    const [i, o] = this.querySelectorAll('aj-pane'); i.data = d.input; o.data = d.output;
  }
}
define('aj-example', Example);

// <aj-ws-toggle for="selector"> — toggles whitespace on every aj-pane matched
class WsToggle extends Base {
  render() {
    this.innerHTML = `<button class="btn sm" type="button" aria-pressed="false">공백 표시</button>`;
    this.querySelector('button').onclick = e => {
      const on = e.currentTarget.getAttribute('aria-pressed') !== 'true'; e.currentTarget.setAttribute('aria-pressed', on);
      document.querySelectorAll(attr(this, 'for', 'aj-pane')).forEach(p => { p.toggleWs(p.querySelector('[data-ws]'), on); });
    };
  }
}
define('aj-ws-toggle', WsToggle);
