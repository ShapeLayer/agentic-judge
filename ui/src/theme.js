// Theme: 'system' | 'light' | 'dark'. Stored in localStorage('aj.theme').
// Applied as data-theme on <html>; 'system' removes the attribute so CSS
// falls back to prefers-color-scheme. Must run before first paint (see
// the inline bootstrap in each page's <head>).
const KEY = 'aj.theme';
const ORDER = ['system', 'light', 'dark'];
const LABEL = { system: '시스템', light: '라이트', dark: '다크' };
const GLYPH = { system: '◐', light: '○', dark: '●' };

export function getTheme() {
  try { const v = localStorage.getItem(KEY); return ORDER.includes(v) ? v : 'system'; } catch { return 'system'; }
}
export function applyTheme(mode) {
  const root = document.documentElement;
  if (mode === 'system') root.removeAttribute('data-theme'); else root.dataset.theme = mode;
  try { mode === 'system' ? localStorage.removeItem(KEY) : localStorage.setItem(KEY, mode); } catch {}
  document.querySelectorAll('aj-theme-switch').forEach(el => el.render());
}
export function resolvedTheme() {
  const mode = getTheme();
  if (mode !== 'system') return mode;
  return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

class ThemeSwitch extends HTMLElement {
  connectedCallback() {
    this.render();
    this.addEventListener('click', () => {
      const next = ORDER[(ORDER.indexOf(getTheme()) + 1) % ORDER.length];
      applyTheme(next);
    });
    matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => this.render());
  }
  render() {
    const mode = getTheme();
    this.innerHTML = `<button class="btn ghost sm" type="button" aria-label="테마: ${LABEL[mode]} (클릭하여 변경)" title="테마: ${LABEL[mode]} · 현재 ${resolvedTheme() === 'dark' ? '다크' : '라이트'}">
      <span aria-hidden="true">${GLYPH[mode]}</span><span>${LABEL[mode]}</span></button>`;
  }
}
customElements.define('aj-theme-switch', ThemeSwitch);
