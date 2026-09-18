import { Base, define, esc, readData } from '../util.js';
// <aj-argv data='["--n","1"]'> — generator arguments as an array. Never joined into a shell string.
// Enter adds the typed value; Backspace on empty input removes the last chip; emits 'change' {detail: string[]}.
class Argv extends Base {
  render() {
    if (!this._items) this._items = [...(readData(this, []) ?? [])];
    this.innerHTML = `<div class="argv" role="list" aria-label="인수 목록">${this._items.map((a, i) => `<span class="chip" role="listitem">${esc(a)}<button type="button" data-i="${i}" aria-label="인수 ${esc(a)} 제거">×</button></span>`).join('')}<input type="text" placeholder="인수 입력 후 Enter" aria-label="인수 추가"></div>`;
    const input = this.querySelector('input');
    this.querySelector('.argv').addEventListener('click', e => { const b = e.target.closest('button[data-i]'); if (b) { this._items.splice(+b.dataset.i, 1); this.emit(); } else input.focus(); });
    input.addEventListener('keydown', e => {
      if (e.key === 'Enter' && input.value !== '') { e.preventDefault(); this._items.push(input.value); this.emit(); this.querySelector('input').focus(); }
      else if (e.key === 'Backspace' && input.value === '' && this._items.length) { this._items.pop(); this.emit(); this.querySelector('input').focus(); }
    });
  }
  emit() { this.render(); this.dispatchEvent(new CustomEvent('change', { bubbles: true, detail: [...this._items] })); }
  get value() { return [...this._items]; }
  set value(v) { this._items = [...v]; this.render(); }
}
define('aj-argv', Argv);
