import { Base, define, attr } from '../util.js';
// <aj-trust valid="2" invalid="0" uncertain="1" errors="0" historical="3" fingerprint="4d0e…9a">
// Counts only for the current fingerprint. No percentages. All attributes absent → "미검증".
export const trustHtml = t => {
  if (!t || ['valid', 'invalid', 'uncertain', 'errors'].every(k => !t[k])) return `<span class="trust" aria-label="AI 검증 없음"><span class="n none">미검증</span></span>`;
  const n = (k, g, cls) => `<span class="n ${cls}" title="${cls}">${g} ${t[k] ?? 0}</span>`;
  return `<span class="trust" aria-label="AI 검증: valid ${t.valid ?? 0}, invalid ${t.invalid ?? 0}, uncertain ${t.uncertain ?? 0}, error ${t.errors ?? 0}">${n('valid', '✓', 'valid')}${n('invalid', '✗', 'invalid')}${n('uncertain', '?', 'uncertain')}${n('errors', '!', 'error')}${t.historical ? `<span class="faint">· 이력 ${t.historical}</span>` : ''}</span>`;
};
class Trust extends Base {
  static observedAttributes = ['valid', 'invalid', 'uncertain', 'errors', 'historical'];
  render() {
    const t = this.data ?? Object.fromEntries(['valid', 'invalid', 'uncertain', 'errors', 'historical'].map(k => [k, Number(attr(this, k, 0))]));
    this.innerHTML = trustHtml(t);
  }
}
define('aj-trust', Trust);
