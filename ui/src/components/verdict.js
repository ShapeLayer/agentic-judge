import { Base, define, esc, attr } from '../util.js';
// <aj-verdict code="AC|WA|PE|RE|TLE|MLE|CE|CHECKER_ERROR|PENDING" size="lg" label="...">
export const VERDICT = {
  AC: ['✓', 'AC', '정답'], WA: ['✗', 'WA', '오답'], PE: ['≈', 'PE', '출력 형식 오류'], RE: ['!', 'RE', '런타임 오류'],
  TLE: ['⏱', 'TLE', '시간 초과'], MLE: ['▣', 'MLE', '메모리 초과'], CE: ['⚙', 'CE', '컴파일 오류'],
  CHECKER_ERROR: ['⚠', 'CHECKER', '채점기 오류 (시스템)'], PENDING: ['…', '대기', '채점 중'],
};
export const verdictHtml = (code, size = '', label = '') => {
  const [g, t, desc] = VERDICT[code] || ['?', code, code];
  return `<span class="verdict${size ? ' ' + size : ''}" data-v="${esc(code)}" title="${esc(desc)}" role="img" aria-label="${esc(t)} — ${esc(desc)}"><span class="g" aria-hidden="true">${g}</span><span>${esc(label || t)}</span></span>`;
};
class Verdict extends Base {
  static observedAttributes = ['code', 'size', 'label'];
  render() { this.innerHTML = verdictHtml(attr(this, 'code', 'PENDING'), attr(this, 'size'), attr(this, 'label')); }
}
define('aj-verdict', Verdict);
