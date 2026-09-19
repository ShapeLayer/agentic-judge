import type { ApiResults, Capabilities, Draft, Manifest } from './types.js';
interface Elements {
  source: HTMLTextAreaElement; filter: HTMLInputElement; file: HTMLInputElement;
  language: HTMLSelectElement; 'answer-language': HTMLSelectElement;
  'statement-form': HTMLFormElement; 'test-form': HTMLFormElement; 'answer-form': HTMLFormElement;
  'workspace-open': HTMLFormElement; create: HTMLFormElement; import: HTMLFormElement;
  setup: HTMLDetailsElement; 'archive-problem': HTMLFormElement; 'archive-tests': HTMLFormElement;
}
function $<K extends string>(id: K): K extends keyof Elements ? Elements[K] : HTMLElement {
  const element = document.getElementById(id);
  if (!element) throw new Error(`Missing UI element: ${id}`);
  return element as K extends keyof Elements ? Elements[K] : HTMLElement;
}
function field(form: HTMLFormElement, name: string): HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement {
  const control = form.elements.namedItem(name);
  if (!(control instanceof HTMLInputElement || control instanceof HTMLTextAreaElement || control instanceof HTMLSelectElement)) throw new Error(`Missing form field: ${name}`);
  return control;
}
const errorMessage = (error: unknown): string => error instanceof Error ? error.message : String(error);
function requireSelected(): string {
  if (!selected) throw new Error('먼저 문제를 선택하세요.');
  return selected;
}
let capabilities: Capabilities, problems: Manifest[] = [], selected: string | null = null, language = '', timer: ReturnType<typeof setTimeout> | undefined, saving: Promise<void> = Promise.resolve(), busy = false;
const notice = (text = '', error = false) => { $('notice').textContent = text; $('notice').className = error ? 'error' : ''; };
async function api<K extends keyof ApiResults>(name: K, args: object = {}): Promise<ApiResults[K]> {
  const body = JSON.stringify(args);
  if (new TextEncoder().encode(body).length > 4*1024*1024) throw new Error('요청 크기가 4 MiB를 초과합니다. 코드를 줄여 주세요.');
  const response = await fetch(`/api/tools/${name}`, {method:'POST', headers:{'Content-Type':'application/json'}, body});
  const data = await response.json();
  if (!response.ok) throw new Error(data.error || `요청에 실패했습니다 (${response.status}).`);
  return data;
}
async function action(work: () => void | Promise<void>) {
  if (busy) return;
  busy = true;
  const controls = [...document.querySelectorAll<HTMLButtonElement | HTMLSelectElement | HTMLInputElement | HTMLTextAreaElement>('button, select, input, textarea')];
  const states = controls.map(el => el.disabled);
  controls.forEach(el => el.disabled = true);
  notice('처리 중입니다.');
  try { await work(); notice(); }
  catch (e) { notice(errorMessage(e), true); }
  finally { controls.forEach((el, i) => el.disabled = states[i]); busy = false; }
}
const bind = (id: string, event: string, work: (event: Event) => void | Promise<void>) => $(id).addEventListener(event, e => { e.preventDefault(); void action(() => work(e)); });
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text: string) => { const el = document.createElement(tag); el.textContent = text; return el; };
const key = (id = selected, lang = language) => JSON.stringify(['agentic-judge.draft', capabilities.storage_root, capabilities.workspace, id, lang]);
function localRead(k: string): unknown { try { return JSON.parse(localStorage.getItem(k) ?? 'null') as unknown; } catch { return null; } }
function localDraft(k: string): Draft | null {
  const value = localRead(k);
  if (typeof value !== 'object' || value === null) return null;
  if ('problem_id' in value && typeof value.problem_id === 'string' && 'language' in value && typeof value.language === 'string' && 'source' in value && typeof value.source === 'string') return {problem_id: value.problem_id, language: value.language, source: value.source};
  return null;
}
function preferredLanguage(id: string) {
  const saved = localRead(key(id, '@language'));
  return typeof saved === 'string' && capabilities.languages[saved] ? saved : language;
}
function rememberLanguage() {
  try { localStorage.setItem(key(selected, '@language'), JSON.stringify(language)); } catch { /* Drafts remain on the server. */ }
}
function snapshot() { return {problem_id:requireSelected(), language, source:$('source').value}; }
function backup() {
  if (!selected) return;
  try { localStorage.setItem(key(), JSON.stringify(snapshot())); }
  catch { $('draft-status').textContent = '브라우저 임시 저장이 불가능합니다. 서버 저장을 기다려 주세요.'; }
}
function saveDraft() {
  clearTimeout(timer);
  if (!selected) return saving;
  const draft = snapshot(), k = key();
  saving = saving.catch(() => {}).then(async () => {
    try {
      await api('draft_save', draft);
      if (localDraft(k)?.source === draft.source) { try { localStorage.removeItem(k); } catch { /* Server copy is durable. */ } }
      if (selected === draft.problem_id && language === draft.language && $('source').value === draft.source) $('draft-status').textContent = '초안을 저장했습니다.';
    } catch (e) {
      $('draft-status').textContent = '서버 저장에 실패했습니다. 파일로 저장하거나 다시 시도하세요.';
      throw e;
    }
  });
  return saving;
}
async function readDraft(id: string, lang: string) {
  const local = localDraft(key(id, lang));
  return {local, draft:local || await api('draft_get', {problem_id:id, language:lang})};
}
function restoreDraft({local, draft}: {local: Draft | null; draft: Draft | null}) {
  $('source').value = draft?.source || '';
  $('draft-status').textContent = local ? '미전송 초안을 복구했습니다.' : draft ? '저장된 초안을 복구했습니다.' : '작성한 코드는 자동 저장됩니다.';
  if (local) void saveDraft().catch(e => notice(errorMessage(e), true));
}
$('source').addEventListener('input', () => { backup(); $('draft-status').textContent = '저장 중입니다…'; clearTimeout(timer); timer = setTimeout(() => void saveDraft().catch(e => notice(errorMessage(e), true)), 500); });
$('source').addEventListener('keydown', e => {
  if (e.key === 'Tab') { e.preventDefault(); const area = e.currentTarget as HTMLTextAreaElement; area.setRangeText('    ', area.selectionStart, area.selectionEnd, 'end'); area.dispatchEvent(new Event('input')); }
});
window.addEventListener('beforeunload', e => {
  if (selected && localRead(key())) { backup(); e.preventDefault(); e.returnValue = ''; }
});
function renderList() {
  const list = $('problems'); list.replaceChildren();
  const filter = $('filter').value.toLowerCase();
  for (const p of problems.filter(p => `${p.name} ${p.id}`.toLowerCase().includes(filter))) {
    const button = node('button', p.name); button.title = p.id;
    if (p.id === selected) button.setAttribute('aria-current', 'page');
    button.onclick = () => void action(() => selectProblem(p.id)); list.append(button);
  }
  if (!list.children.length) list.append(node('p', '표시할 문제가 없습니다.'));
}
async function refresh() { problems = await api('problems_list'); renderList(); }
async function renderProblem() {
  const p = await api('problem_get', {problem_id:requireSelected()});
  $('problem-name').textContent = p.manifest.name;
  $('limits').textContent = `${p.manifest.time_limit_ms} ms · ${p.manifest.memory_limit_mb} MiB · ${selected}`;
  $('readiness').textContent = p.ready ? '채점 준비 완료' : '채점 준비가 필요합니다';
  const statement = $('statement'); statement.replaceChildren();
  for (const [section, title] of [['legend','문제'],['input','입력'],['output','출력'],['notes','참고']]) {
    if (!p.statements[section].markdown) continue;
    statement.append(node('h2', title));
    // HTML is sanitized by the server; no user code or scripts are inserted.
    const content = document.createElement('div'); content.innerHTML = p.statements[section].html; statement.append(content);
  }
  if (!statement.children.length) statement.append(node('p', '등록된 문제 설명이 없습니다.'));
  const examples = $('examples'); examples.replaceChildren();
  for (const example of p.examples) examples.append(node('h2', `예제 ${example.number}`), node('pre', example.input), node('pre', example.output));
  field($('statement-form'), 'markdown').value = p.statements.legend.markdown;
  field($('test-form'), 'number').value = String(Math.max(0, ...p.tests.filter(t => t.suite === 'general').map(t => t.number)) + 1);
  const archives = $('test-archives'); archives.replaceChildren();
  for (const suite of p.manifest.suites) {
    const group = node('div', ''); group.className = 'toolbar'; group.append(node('strong', suite));
    const download = (label: string, args: Record<string, string>) => { const button = node('button', label); button.onclick = () => void action(() => downloadArchive(args)); return button; };
    group.append(download('테스트 셋 ZIP 내보내기', {kind:'suite', problem_id:requireSelected(), suite}));
    for (const test of p.tests.filter(t => t.suite === suite)) group.append(download(`#${test.number} ZIP 내보내기`, {kind:'test', problem_id:requireSelected(), suite, number:String(test.number)}));
    if (!p.tests.some(t => t.suite === suite)) group.append(node('span', '테스트 없음'));
    archives.append(group);
  }
  await renderSubmissions();
}
async function renderSubmissions() {
  const attempts = await api('submissions_list', {problem_id:requireSelected()});
  const target = $('submissions'); target.replaceChildren();
  attempts.sort((a,b) => b.created_at - a.created_at);
  for (const s of attempts) {
    const detail = document.createElement('details');
    detail.append(node('summary', `${s.verdict} · ${s.language} · ${new Date(s.created_at*1000).toLocaleString()}`));
    detail.append(node('pre', s.source));
    for (const c of s.cases) {
      detail.append(node('p', `${c.number ? `${c.suite} #${c.number}` : '컴파일'} · ${c.result.verdict} · ${c.result.time_ms} ms`));
      for (const [label, value] of [['입력',c.input],['기대 출력',c.expected_output],['실행 출력',c.result.stdout],['진단',c.result.stderr],['체커',c.checker_message]] as const) if (value) detail.append(node('strong',label),node('pre',value));
    }
    target.append(detail);
  }
  if (!attempts.length) target.append(node('p', '아직 제출한 풀이가 없습니다.'));
}
async function selectProblem(id: string) {
  await saveDraft();
  const nextLanguage = preferredLanguage(id);
  const restored = await readDraft(id, nextLanguage);
  selected = id; language = nextLanguage;
  $('language').value = language;
  restoreDraft(restored);
  history.replaceState(null, '', `#${encodeURIComponent(id)}`);
  $('empty').hidden = true; $('problem').hidden = false;
  $('answer-form').reset(); $('answer-language').value = language;
  await renderProblem(); renderList();
}
bind('language','change', async () => {
  const next = $('language').value;
  try { await saveDraft(); const restored = await readDraft(requireSelected(), next); language = next; rememberLanguage(); restoreDraft(restored); }
  catch (e) { $('language').value = language; throw e; }
});
async function openSource(name: string, source: string) {
  if (new TextEncoder().encode(source).length > 3*1024*1024) throw new Error('파일은 3 MiB 이하여야 합니다.');
  await saveDraft();
  const ext = name.split('.').pop()?.toLowerCase();
  const match = Object.entries(capabilities.languages).find(([,recipe]) => recipe.filename.split('.').pop()?.toLowerCase() === ext);
  if (match) { language = match[0]; $('language').value = language; rememberLanguage(); }
  $('source').value = source; backup(); await saveDraft();
}
bind('file','change', async () => { const file = $('file').files?.[0]; if (!file) return; if (file.size > 3*1024*1024) throw new Error('파일은 3 MiB 이하여야 합니다.'); await openSource(file.name, await file.text()); $('file').value = ''; });
bind('download','click', async () => {
  await saveDraft();
  const link = document.createElement('a');
  link.href = `/api/draft-download?${new URLSearchParams({problem_id:requireSelected(), language})}`;
  link.download = capabilities.languages[language].filename;
  document.body.append(link); link.click(); link.remove();
});
bind('submit','click', async () => {
  await saveDraft();
  try { await api('submit', snapshot()); }
  catch (e) { await renderSubmissions().catch(() => {}); throw new Error(`${errorMessage(e)} 제출 기록을 확인한 뒤 다시 시도하세요.`); }
  await renderSubmissions(); $('submissions').firstElementChild?.setAttribute('open','');
});
bind('workspace-open','submit', async () => { const path = field($('workspace-open'), 'path').value; const result = await api('workspace_read',{path}); await openSource(path, result.content); });
bind('refresh','click', async () => { await saveDraft(); await refresh(); if (selected) await renderProblem(); });
$('filter').addEventListener('input',renderList);
bind('create','submit', async () => { const args = {problem_id:field($('create'), 'problem_id').value, name:field($('create'), 'name').value}; await api('problem_create',args); await refresh(); await selectProblem(args.problem_id); $('setup').open = true; });
bind('import','submit', async () => { const args = {problem_id:field($('import'), 'problem_id').value, path:field($('import'), 'path').value}; const result = await api('problem_import',args); await refresh(); await selectProblem(args.problem_id); if (result.warnings?.length) $('readiness').textContent += ` · ${result.warnings.join(' · ')}`; });
bind('statement-form','submit', async () => { await api('statement_set',{problem_id:requireSelected(), section:'legend', markdown:field($('statement-form'), 'markdown').value}); await renderProblem(); });
bind('answer-form','submit', async () => {
  const form = $('answer-form');
  const result = await api('answer_save',{problem_id:requireSelected(),language:field(form, 'language').value,source:field(form, 'source').value,expected:'AC',main:true});
  await renderProblem(); if (result.regeneration?.error) throw new Error(result.regeneration.error);
});
bind('test-form','submit', async () => { const form = $('test-form'); await api('test_put',{problem_id:requireSelected(),suite:field(form, 'suite').value,number:Number(field(form, 'number').value),input:field(form, 'input').value}); await renderProblem(); });
async function downloadArchive(args: Record<string, string>) {
  const response = await fetch(`/api/archive?${new URLSearchParams(args)}`);
  if (!response.ok) throw new Error((await response.json()).error);
  const url = URL.createObjectURL(await response.blob());
  const link = document.createElement('a'); link.href = url;
  link.download = [args.problem_id, args.kind, args.suite, args.number].filter(Boolean).join('-') + '.zip';
  document.body.append(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 60000);
}
async function uploadArchive(form: HTMLFormElement, args: Record<string, string>) {
  const input = field(form, 'archive') as HTMLInputElement;
  const file = input.files?.[0]; if (!file) throw new Error('ZIP 파일을 선택하세요.');
  if (file.size > 32*1024*1024) throw new Error('ZIP 파일은 32 MiB 이하여야 합니다.');
  const response = await fetch(`/api/archive?${new URLSearchParams(args)}`, {method:'POST', headers:{'Content-Type':'application/zip'}, body:file});
  const result = await response.json(); if (!response.ok) throw new Error(result.error);
  input.value = '';
}
bind('regenerate-tests', 'click', async () => { await api('regenerate', {problem_id:requireSelected()}); await renderProblem(); });
bind('export-problem', 'click', () => downloadArchive({kind:'problem', problem_id:requireSelected()}));
bind('archive-problem', 'submit', async () => { const form = $('archive-problem'); const id = field(form, 'problem_id').value; await uploadArchive(form, {kind:'problem', problem_id:id}); await refresh(); await selectProblem(id); });
field($('archive-tests'), 'kind').addEventListener('change', () => { $('archive-number').hidden = field($('archive-tests'), 'kind').value !== 'test'; });
bind('archive-tests', 'submit', async () => { const form = $('archive-tests'); const kind = field(form, 'kind').value; await uploadArchive(form, {kind, problem_id:requireSelected(), suite:field(form, 'suite').value, ...(kind === 'test' ? {number:field(form, 'number').value} : {})}); await renderProblem(); });
await action(async () => {
  capabilities = await api('capabilities');
  $('workspace').textContent = capabilities.workspace ? `워크스페이스 · ${capabilities.workspace}` : '즉석 작성 · 폴더 연결 없음';
  $('import-panel').hidden = !capabilities.workspace;
  $('workspace-files').hidden = !capabilities.workspace;
  for (const id of Object.keys(capabilities.languages)) for (const select of [$('language'),$('answer-language')]) { const option = node('option',id); option.value=id; select.append(option); }
  language = capabilities.languages.python ? 'python' : Object.keys(capabilities.languages)[0];
  $('language').value = language;
  await refresh();
  if (capabilities.workspace) {
    const entries = await api('workspace_list');
    for (const file of entries.filter(f => !f.directory)) { const button=node('button',file.path); button.onclick=()=>void action(async()=>{ if (!selected) throw new Error('먼저 문제를 선택하세요.'); const r=await api('workspace_read',{path:file.path}); await openSource(file.path,r.content); }); $('files').append(button); }
  }
  const id = decodeURIComponent(location.hash.slice(1));
  if (problems.some(p => p.id === id)) await selectProblem(id);
});
