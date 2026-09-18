// Shared helpers for light-DOM components.
export const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
export const attr = (el, name, fallback = '') => el.hasAttribute(name) ? el.getAttribute(name) : fallback;

/** Read JSON from `el.data`, a `data` attribute, or a child <script type="application/json">. */
export function readData(el, fallback = null) {
  if (el.data !== undefined) return el.data;
  const script = el.querySelector(':scope > script[type="application/json"]');
  try {
    if (script) return JSON.parse(script.textContent);
    if (el.hasAttribute('data')) return JSON.parse(el.getAttribute('data'));
  } catch (e) { console.error(el.tagName, 'invalid JSON data', e); }
  return fallback;
}

export const fmtMs = ms => ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${ms} ms`;
export const fmtKb = kb => kb === 0 ? '측정 불가' : kb >= 1024 ? `${(kb / 1024).toFixed(1)} MB` : `${kb} KB`;
export const fmtBytes = b => b >= 1048576 ? `${(b / 1048576).toFixed(1)} MiB` : b >= 1024 ? `${(b / 1024).toFixed(1)} KB` : `${b} B`;
export const fmtTime = unix => { const d = new Date(unix * 1000); const p = n => String(n).padStart(2, '0'); return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`; };
export const short = id => String(id ?? '').slice(0, 8);
export const fill = (tpl, obj) => tpl.replace(/\{(\w+)\}/g, (_, k) => encodeURIComponent(obj[k] ?? ''));

/** Base: render on connect, re-render on observed attribute change (after first render). */
export class Base extends HTMLElement {
  connectedCallback() { if (!this._ready) { this._ready = true; this.render(); } }
  attributeChangedCallback() { if (this._ready) this.render(); }
  set data(v) { this._data = v; if (this._ready) this.render(); }
  get data() { return this._data; }
}
export const define = (name, cls) => { if (!customElements.get(name)) customElements.define(name, cls); };
