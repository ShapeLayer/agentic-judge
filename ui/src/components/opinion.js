import { Base, define, esc, attr } from '../util.js';
// <aj-opinion status="valid|invalid|uncertain|error" provider="remote" requested-model="m" response-model="m" at="14:33" rev="3" stale direction="…">rationale</aj-opinion>
// Model display rules (UI-SPEC): response_model shown as 응답 모델; if it differs from requested_model, show both;
// no response_model → requested as 요청 모델 + notice; neither → "모델 정보가 기록되지 않았습니다."
const STATUS = { valid: ['ok', 'valid'], invalid: ['err', 'invalid'], uncertain: ['warn', 'uncertain'], error: ['system', 'error'] };
export function modelHtml(requested, response) {
  if (response && requested && response !== requested) return `<span class="muted">응답 모델 <span class="mono">${esc(response)}</span> · 요청 모델 <span class="mono">${esc(requested)}</span></span>`;
  if (response) return `<span class="muted">응답 모델 <span class="mono">${esc(response)}</span></span>`;
  if (requested) return `<span class="muted" title="응답 모델 정보가 제공되지 않았습니다.">요청 모델 <span class="mono">${esc(requested)}</span> <span class="faint">(응답 모델 정보 없음)</span></span>`;
  return `<span class="faint">모델 정보가 기록되지 않았습니다.</span>`;
}
class Opinion extends Base {
  static observedAttributes = ['status', 'provider', 'requested-model', 'response-model', 'at', 'rev', 'stale', 'direction'];
  render() {
    if (this._body === undefined) this._body = this.data?.rationale ?? this.innerHTML.trim();
    const d = this.data ?? {};
    const status = d.status ?? attr(this, 'status', 'uncertain');
    const [cls, label] = STATUS[status] || ['', status];
    const stale = this.hasAttribute('stale') || d.stale;
    const direction = d.suggested_direction ?? attr(this, 'direction');
    const rev = d.revision ?? attr(this, 'rev');
    this.innerHTML = `<div class="opinion${stale ? ' stale' : ''}">
      <div class="head"><span class="badge ${cls}">${esc(label)}</span><span class="mono">${esc(d.provider ?? attr(this, 'provider'))}</span>${modelHtml(d.requested_model ?? attr(this, 'requested-model'), d.response_model ?? attr(this, 'response-model'))}<span class="muted">${rev ? `rev ${esc(rev)} · ` : ''}${esc(d.at ?? attr(this, 'at'))}${stale ? ' · 이전 리비전 — 집계 제외' : ''}</span></div>
      <div class="body${status === 'error' ? ' mono xs' : ''}">${this._body}</div>
      ${direction ? `<div class="dir">수정 방향: ${esc(direction)} <span class="faint">(AI는 코드를 수정하지 않습니다)</span></div>` : ''}</div>`;
  }
}
define('aj-opinion', Opinion);
