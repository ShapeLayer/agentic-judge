import { Base, define, esc, attr } from '../util.js';
// <aj-status kind="ok|warn|err|info" title="✓ 채점 가능" detail="테스트 12"> <button>…</button> </aj-status>
// Child elements become the action area. Readiness line: only server `ready` is trusted.
class Status extends Base {
  static observedAttributes = ['kind', 'title', 'detail'];
  render() {
    if (!this._actions) this._actions = [...this.children];
    this.setAttribute('role', attr(this, 'kind') === 'err' ? 'alert' : 'status');
    this.innerHTML = `<div class="status ${esc(attr(this, 'kind', 'info'))}"><span class="title">${esc(attr(this, 'title'))}</span>${this.hasAttribute('detail') ? `<span class="muted">${this.getAttribute('detail')}</span>` : ''}<span class="actions"></span></div>`;
    this.querySelector('.actions').append(...this._actions);
  }
}
define('aj-status', Status);

// <aj-banner message="HTTP 400 · problem not found">AI 프로바이더 연결 실패</aj-banner> → dispatches 'retry'
class Banner extends Base {
  static observedAttributes = ['message', 'note'];
  render() {
    if (this._label === undefined) this._label = this.textContent.trim();
    this.setAttribute('role', 'alert');
    this.innerHTML = `<div class="banner"><span>✗ ${esc(this._label)}</span>${this.hasAttribute('message') ? `<span class="msg">${esc(this.getAttribute('message'))}</span>` : ''}${this.hasAttribute('note') ? `<span class="xs muted">${esc(this.getAttribute('note'))}</span>` : ''}<span class="actions"><button class="btn sm" type="button">다시 시도</button></span></div>`;
    this.querySelector('button').onclick = () => this.dispatchEvent(new CustomEvent('retry', { bubbles: true }));
  }
}
define('aj-banner', Banner);
