import { Base, define, esc, attr, fmtBytes } from '../util.js';
// <aj-editor language="python" filename="main.py" dirty>source</aj-editor>
// Design-phase shell where Monaco mounts. `value` property mirrors the editor content; emits 'change'.
// Very small tokenizer only for the mockup; Monaco replaces .editor-body entirely.
const KW = 'import|from|def|return|if|else|elif|for|while|in|int|long|using|namespace|include|const|let|var|fn|pub|struct|class|print|cin|cout|std|void|main|auto';
// Single-pass tokenizer: each match is escaped once, so inserted markup is never re-scanned.
const tokenize = (line, lang) => {
  const hash = /^(python|bash|ruby|php|markdown)$/i.test(lang);
  const re = new RegExp(`(${hash ? '#.*' : '\\/\\/.*'})|("[^"]*"|'[^']*')|\\b(${KW})\\b|\\b(\\d+)\\b|([^]+?)(?=${hash ? '#' : '\\/\\/'}|["']|\\b(?:${KW})\\b|\\b\\d|$)`, 'g');
  return line.replace(re, (m, cm, str, kw, num, rest) =>
    cm ? `<span class="tok-cm">${esc(cm)}</span>` : str ? `<span class="tok-str">${esc(str)}</span>` : kw ? `<span class="tok-kw">${esc(kw)}</span>` : num ? `<span class="tok-num">${esc(num)}</span>` : esc(rest ?? m));
};
class Editor extends Base {
  static observedAttributes = ['language', 'filename', 'dirty', 'readonly', 'min-height'];
  render() {
    if (this._value === undefined) this._value = this.data ?? this.textContent.replace(/^\n/, '');
    const lines = this._value.split('\n'); if (lines[lines.length - 1] === '') lines.pop();
    const n = Math.max(lines.length, 1);
    this.innerHTML = `<div class="editor" style="min-height:${esc(attr(this, 'min-height', '360px'))}" role="textbox" aria-multiline="true" aria-label="코드 에디터" tabindex="0"${this.hasAttribute('readonly') ? ' aria-readonly="true"' : ''}>
      <div class="editor-body"><div class="editor-gutter">${Array.from({ length: n }, (_, i) => `<span>${i + 1}</span>`).join('')}</div><pre class="editor-code">${lines.map(l => tokenize(l, attr(this, 'language'))).join('\n')}\n</pre></div>
      <div class="editor-status"><span>${esc(attr(this, 'language'))}</span><span>UTF-8</span><span>LF</span>${this.hasAttribute('dirty') ? '<span class="dirty">● 저장되지 않음</span>' : ''}<span class="grow"></span><span class="num">${fmtBytes(new TextEncoder().encode(this._value).length)}</span><span class="num">Ln ${n}, Col ${(lines[n - 1] ?? '').length + 1}</span></div></div>`;
  }
  get value() { return this._value; }
  set value(v) { this._value = v; this.render(); this.dispatchEvent(new CustomEvent('change', { bubbles: true, detail: { value: v } })); }
  get bytes() { return new TextEncoder().encode(this._value).length; }
}
define('aj-editor', Editor);
