import * as monaco from 'monaco-editor/esm/vs/editor/editor.api.js';
import 'monaco-editor/esm/vs/basic-languages/monaco.contribution.js';
import type { ApiResults, Capabilities, Draft, TextPreview, SubmissionSummary, SubmissionPreview, Problem, Answer } from './types.js';
interface Elements {
  'loading-overlay': HTMLDialogElement; 'loading-progress': HTMLProgressElement;
  'case-dialog': HTMLDialogElement; 'problem-picker': HTMLDialogElement; 'history-dialog': HTMLDialogElement; 'history-search': HTMLFormElement; 'history-query': HTMLInputElement; 'history-verdict': HTMLSelectElement; 'history-language': HTMLSelectElement; filter: HTMLInputElement; file: HTMLInputElement;
  language: HTMLSelectElement; 'answer-language': HTMLSelectElement;
  'results-filter': HTMLFormElement; 'results-verdict': HTMLSelectElement; 'results-language': HTMLSelectElement; 'statement-form': HTMLFormElement; 'test-form': HTMLFormElement; 'answer-form': HTMLFormElement;
  'workspace-open': HTMLFormElement; create: HTMLFormElement; import: HTMLFormElement;
  setup: HTMLElement; 'validator-form': HTMLFormElement; 'archive-problem': HTMLFormElement; 'archive-tests': HTMLFormElement;
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
type LoadStep = (text: string) => void;
async function withLoading(title: string, total: number, work: (step: LoadStep) => Promise<void>) {
  const overlay = $('loading-overlay');
  if (overlay.open) overlay.close();
  $('loading-title').textContent = title;
  let completed = 0;
  const update = () => {
    const percent = Math.round(completed / total * 100);
    $('loading-progress').value = percent;
    $('loading-count').textContent = `${completed} / ${total}단계 완료`;
    $('loading-percent').textContent = `${percent}%`;
  };
  update(); overlay.showModal();
  document.querySelector('main')?.setAttribute('aria-busy', 'true');
  let started = false;
  const step: LoadStep = text => {
    if (started) completed++;
    started = true;
    $('loading-step').textContent = text;
    update();
  };
  try {
    await work(step);
    completed = total; update();
  } finally {
    overlay.close();
    document.querySelector('main')?.removeAttribute('aria-busy');
  }
}
$('loading-overlay').addEventListener('cancel', event => event.preventDefault());
const problemLoadSteps = 3;
let currentProblem: Problem | null = null;
let preparationOffset = 0, setupVersion = 0;
let setupCache: {id: string; answers: Answer[]; validators: ApiResults['validator_runs_list']; modelItems: ApiResults['artifact_models']} | null = null;
const errorMessage = (error: unknown): string => error instanceof Error ? error.message : String(error);
function requireSelected(): string {
  if (!selected) throw new Error('먼저 문제를 선택하세요.');
  return selected;
}
(self as typeof self & {MonacoEnvironment: monaco.Environment}).MonacoEnvironment = {getWorker: () => new Worker('/editor.worker.js', {type:'module'})};
const editor = monaco.editor.create($('source'), {value:'', language:'plaintext', automaticLayout:true, minimap:{enabled:false}, fontSize:14, tabSize:4, scrollBeyondLastLine:false, ariaLabel:'풀이 코드'});
const answerEditor = monaco.editor.create($('answer-source'), {value:'', language:'plaintext', automaticLayout:true, minimap:{enabled:false}, fontSize:14, tabSize:4, scrollBeyondLastLine:false, ariaLabel:'채점 준비 풀이 코드'});
const dark = matchMedia('(prefers-color-scheme: dark)');
const updateTheme = () => monaco.editor.setTheme(dark.matches ? 'vs-dark' : 'vs');
updateTheme(); dark.addEventListener('change', updateTheme);
const setEditorLanguage = (target: monaco.editor.IStandaloneCodeEditor, lang: string) => { const ext = capabilities.languages[lang]?.filename.split('.').pop() || ''; monaco.editor.setModelLanguage(target.getModel()!, ({py:'python',cpp:'cpp',cc:'cpp',c:'c',rs:'rust',js:'javascript',ts:'typescript',java:'java',kt:'kotlin',go:'go',cs:'csharp',rb:'ruby',swift:'swift'} as Record<string,string>)[ext] || 'plaintext'); };
let restoring = false;
function setSource(source: string) { restoring = true; editor.setValue(source); setEditorLanguage(editor, language); restoring = false; }
let capabilities: Capabilities,  selected: string | null = null, language = '', timer: ReturnType<typeof setTimeout> | undefined, saving: Promise<void> = Promise.resolve(), busy = false;
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
  busy = true; editor.updateOptions({readOnly:true}); answerEditor.updateOptions({readOnly:true});
  const controls = [...document.querySelectorAll<HTMLButtonElement | HTMLSelectElement | HTMLInputElement | HTMLTextAreaElement>('button, select, input, textarea')];
  const states = controls.map(el => el.disabled);
  controls.forEach(el => el.disabled = true);
  notice('처리 중입니다.');
  try { await work(); notice(); }
  catch (e) { notice(errorMessage(e), true); }
  finally { controls.forEach((el, i) => el.disabled = states[i]); busy = false; editor.updateOptions({readOnly:false}); answerEditor.updateOptions({readOnly:false}); updatePagination(); updateHistoryPagination(); }
}
const bind = (id: string, event: string, work: (event: Event) => void | Promise<void>) => $(id).addEventListener(event, e => { e.preventDefault(); void action(() => work(e)); });
const node = <K extends keyof HTMLElementTagNameMap>(tag: K, text: string) => { const el = document.createElement(tag); el.textContent = text; return el; };
const key = (id = selected, lang = language) => JSON.stringify(['agentic-judge.draft', capabilities.storage_root, capabilities.workspace, id, lang]);
const sourceModels = new Map<string, {model: monaco.editor.ITextModel; view: monaco.editor.ICodeEditorViewState | null}>();
let activeSourceKey: string | null = null;
function activateSource(source: string) {
  const nextKey = key();
  if (activeSourceKey === nextKey) { setSource(source); return; }
  if (activeSourceKey) {
    const previous = sourceModels.get(activeSourceKey);
    if (previous) previous.view = editor.saveViewState();
  }
  let next = sourceModels.get(nextKey);
  if (!next) {
    next = {model:monaco.editor.createModel(source, 'plaintext'), view:null};
    sourceModels.set(nextKey, next);
  } else if (next.model.getValue() !== source) {
    next.model.setValue(source);
  }
  const initialModel = activeSourceKey === null ? editor.getModel() : null;
  restoring = true;
  editor.setModel(next.model);
  setEditorLanguage(editor, language);
  if (next.view) editor.restoreViewState(next.view);
  activeSourceKey = nextKey;
  restoring = false;
  initialModel?.dispose();
}
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
function snapshot() { return {problem_id:requireSelected(), language, source:editor.getValue()}; }
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
      if (selected === draft.problem_id && language === draft.language && editor.getValue() === draft.source) $('draft-status').textContent = '초안을 저장했습니다.';
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
  activateSource(draft?.source ?? '');
  $('draft-status').textContent = local ? '미전송 초안을 복구했습니다.' : draft ? '저장된 초안을 복구했습니다.' : '작성한 코드는 자동 저장됩니다.';
  if (local) void saveDraft().catch(e => notice(errorMessage(e), true));
}
editor.onDidChangeModelContent(() => { if (restoring || busy) return; backup(); $('draft-status').textContent = '저장 중입니다…'; clearTimeout(timer); timer = setTimeout(() => void saveDraft().catch(e => notice(errorMessage(e), true)), 500); });
window.addEventListener('beforeunload', e => {
  if (selected && localRead(key())) { backup(); e.preventDefault(); e.returnValue = ''; }
});
let page = 0, total = 0, searchVersion = 0;
const pageSize = 50;
async function renderList() {
  const version = ++searchVersion;
  const result = await api('problems_search', {query:$('filter').value, offset:page*pageSize, limit:pageSize});
  if (version !== searchVersion) return;
  total = result.total;
  const list = $('problems'); list.replaceChildren();
  for (const p of result.items) {
    const button = node('button', p.name); button.append(node('small', p.id));
    if (p.id === selected) button.setAttribute('aria-current', 'page');
    button.onclick = () => void action(async () => { await selectProblem(p.id); $('problem-picker').close(); editor.focus(); }); list.append(button);
  }
  if (!result.items.length) list.append(node('p', '검색 결과가 없습니다. 다른 번호나 이름을 입력하세요.'));
  $('problem-count').textContent = `${total.toLocaleString()}개 문제 · 한 페이지에 ${pageSize}개 표시`;
  $('page-status').textContent = `${page+1} / ${Math.max(1,Math.ceil(total/pageSize))}`;
  updatePagination();
}
function updatePagination() {
  ($('previous-page') as HTMLButtonElement).disabled = page === 0;
  ($('next-page') as HTMLButtonElement).disabled = (page+1)*pageSize >= total;
}
async function refresh() { page = 0; await renderList(); }
function openPicker() { $('problem-picker').showModal(); $('filter').focus(); }
$('choose-problem').onclick = openPicker; $('start-picker').onclick = openPicker;
$('close-picker').onclick = () => $('problem-picker').close();
$('previous-page').onclick = () => { if (page > 0) { page--; void renderList().catch(e => notice(errorMessage(e),true)); } };
$('next-page').onclick = () => { if ((page+1)*pageSize < total) { page++; void renderList().catch(e => notice(errorMessage(e),true)); } };
async function renderProblem(step?: LoadStep): Promise<void> {
  if (!step) return withLoading('문제를 불러오는 중입니다', problemLoadSteps, renderProblem);
  step(`${requireSelected()} · 문제 설명, 예제, 테스트 목록과 채점 준비 상태를 확인하는 중입니다.`);
  const p = await api('problem_get', {problem_id:requireSelected()});
  step('문제 설명과 예제를 화면에 구성하는 중입니다.');
  currentProblem = p;
  if (!field($('answer-form'),'answer_id').value && !answerEditor.getValue()) (field($('answer-form'),'main') as HTMLInputElement).checked=!p.manifest.main_solution;
  renderSetupOverview(p);
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
  renderTestTable(p);
  step('최근 제출 기록을 불러오는 중입니다.');
  await renderSubmissions();
  if (!$('panel-setup').hidden) await renderSetup();
}
let submissionOffset = 0, submissionVersion = 0;
function appendPreview(target: HTMLElement, label: string, preview: TextPreview) {
  if (!preview.bytes) return;
  target.append(node('strong',label), node('pre',preview.text));
  if (preview.truncated) target.append(node('p', `앞 4,096자만 표시합니다 (전체 ${preview.bytes.toLocaleString()}바이트). 나머지는 아래 파일 경로에서 확인하세요.`));
}
type WorkTab = 'solve' | 'results' | 'setup';
const workTabs: WorkTab[] = ['solve','results','setup'];
function showTab(tab: WorkTab) {
  for (const name of workTabs) {
    const active = name === tab;
    $(`tab-${name}`).setAttribute('aria-selected',String(active)); $(`tab-${name}`).tabIndex = active ? 0 : -1;
    $(`panel-${name}`).hidden = !active;
  }
  if (tab === 'solve') editor.layout();
  if (tab === 'setup') { answerEditor.layout(); void renderSetup().catch(e => { $('setup-status').textContent = errorMessage(e); }); }
}
for (const [index, name] of workTabs.entries()) {
  $(`tab-${name}`).onclick = () => showTab(name);
  $(`tab-${name}`).addEventListener('keydown', event => {
    let next: number | undefined;
    if (event.key === 'ArrowRight') next = (index+1)%workTabs.length;
    if (event.key === 'ArrowLeft') next = (index+workTabs.length-1)%workTabs.length;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = workTabs.length-1;
    if (next !== undefined) { event.preventDefault(); showTab(workTabs[next]); $(`tab-${workTabs[next]}`).focus(); }
  });
}
const verdictNames: Record<string,string> = {JUDGING:'채점 중',JUDGE_ERROR:'채점 오류',AC:'통과',WA:'오답',TLE:'시간 초과',MLE:'메모리 초과',RE:'런타임 오류',CE:'컴파일 오류',PE:'출력 형식 오류',CHECKER_ERROR:'체커 오류'};
function verdictBadge(verdict: string) {
  const badge = node('span', verdict === 'JUDGING' ? '채점 중' : `${verdict} · ${verdictNames[verdict] || '기타 판정'}`);
  badge.className = 'verdict'; badge.dataset.verdict = verdict; return badge;
}
function resultTable(headers: string[]) {
  const wrapper = node('div',''); wrapper.className = 'table-scroll';
  const table = document.createElement('table'); table.className = 'results-table';
  const head = table.createTHead().insertRow();
  for (const label of headers) { const cell = node('th',label); cell.scope='col'; head.append(cell); }
  const body = table.createTBody(); wrapper.append(table); return {wrapper,body};
}
let caseVersion = 0;
$('close-case').onclick = () => $('case-dialog').close();
$('case-dialog').addEventListener('close', () => { ++caseVersion; });
function showCase(s: SubmissionSummary, c: SubmissionPreview['cases'][number]) {
  ++caseVersion;
  $('case-title').textContent = c.number ? `${c.suite} #${c.number} · 테스트케이스 상세` : '컴파일 결과 상세';
  const content = $('case-content'); content.replaceChildren(verdictBadge(c.verdict),node('p',`${s.problem_name} · ${s.language} · ${new Date(s.created_at*1000).toLocaleString()}`),node('p',`실행 시간 ${c.time_ms.toLocaleString()} ms · 메모리 ${c.memory_kb.toLocaleString()} KiB · 종료 코드 ${c.exit_code ?? '없음'}`));
  const version = caseVersion;
  for (const [label, field] of [['입력','input'],['기대 출력','expected_output'],['실행 출력','stdout'],['진단','stderr'],['체커 메시지','checker']] as const) {
    if (!c.number && (field === 'input' || field === 'expected_output')) continue;
    const data = document.createElement('details'); data.append(node('summary', `${label} 보기`));
    let fetched = false, fetching = false;
    const body = node('div',''); data.append(body);
    data.addEventListener('toggle', () => {
      if (!data.open || fetched || fetching) return;
      fetching = true; body.textContent='불러오는 중입니다.';
      void api('submission_data_preview', {problem_id:s.problem_id, submission_id:s.id, case_index:c.index, field}).then(preview => {
        if (caseVersion !== version) return;
        body.replaceChildren(); appendPreview(body,label,preview);
        if (field === 'checker') body.append(node('p', '문제의 공통 체커가 이 제출에 대해 남긴 메시지입니다. 체커 소스나 실행 파일이 아닙니다.'));
        if (!preview.exists) body.append(node('p','파일이 삭제되었거나 더 이상 존재하지 않습니다. 평가 결과는 유지됩니다.'));
        else if (!preview.bytes) body.append(node('p',field === 'checker' ? '이 테스트에 대한 체커 메시지가 없습니다.' : '빈 파일입니다.'));
        if (preview.current_test) body.append(node('p','현재 테스트 파일입니다. 제출 이후 테스트가 수정되었을 수 있습니다.'));
        const path=node('code',preview.path);path.className='data-path';body.append(node('strong','실제 파일 경로'),path);fetched=true;
      }).catch(e => {if(caseVersion===version) body.textContent=errorMessage(e);}).finally(()=>{fetching=false;});
    });
    content.append(data);
  }
  $('case-dialog').showModal();
}
let detailVersion = 0;
function memoryLabel(kib: number | null | undefined) { return kib == null ? '—' : `${(kib/1024).toLocaleString(undefined,{maximumFractionDigits:1})} MiB`; }
function sizeLabel(bytes: number | null | undefined) { return bytes == null ? '—' : bytes>=1024 ? `${(bytes/1024).toLocaleString(undefined,{maximumFractionDigits:1})} KiB` : `${bytes.toLocaleString()} B`; }
function submissionTime(timestamp: number) {
  const age=Math.max(0,Math.floor(Date.now()/1000-timestamp));
  const text=age<60 ? '방금 전' : age<3600 ? `${Math.floor(age/60)}분 전` : age<86400 ? `${Math.floor(age/3600)}시간 전` : new Date(timestamp*1000).toLocaleDateString();
  const time=node('time',text); time.dateTime=new Date(timestamp*1000).toISOString(); time.title=new Date(timestamp*1000).toLocaleString(); return time;
}
async function showSubmission(s: SubmissionSummary, target: HTMLElement) {
  target.dataset.submissionId = s.id;
  const version = ++detailVersion;
  target.hidden = false; target.textContent = '테스트별 채점 결과를 불러오는 중입니다.';
  try {
    const record = await api('submission_preview', {problem_id:s.problem_id, submission_id:s.id});
    if (version !== detailVersion || !target.isConnected) return;
    const heading=node('div',''); heading.className='results-heading';
    heading.append(node('h2',`제출 ${s.id.slice(0,8)}`),verdictBadge(record.verdict));
    const close=node('button','상세 닫기'); close.onclick=()=>{++detailVersion;target.hidden=true;}; heading.append(close);
    target.replaceChildren(heading,node('p',`${s.problem_name || s.problem_id} · ${s.language} · ${new Date(s.created_at*1000).toLocaleString()}`));
    const cases=record.cases.filter(c=>c.number>0);
    const metrics=node('div',''); metrics.className='result-metrics';
    const time=record.cases.length ? Math.max(...record.cases.map(c=>c.time_ms)) : null;
    const memory=record.cases.length ? Math.max(...record.cases.map(c=>c.memory_kb)) : null;
    for (const [label,value] of [['통과한 테스트',cases.length ? `${cases.filter(c=>c.verdict==='AC').length} / ${cases.length}` : '—'],['최대 실행 시간',time==null ? '—' : `${time.toLocaleString()} ms`],['최대 메모리',memoryLabel(memory)],['코드 크기',sizeLabel(new TextEncoder().encode(record.source).length)]]) {
      const card=node('div','');card.className='result-metric';card.append(node('span',label),node('strong',value));metrics.append(card);
    }
    target.append(metrics);
    if (s.purpose==='preparation') target.append(node('p',s.preparation?.expected ? `기대 판정: ${s.preparation.expected}` : '이전 검증 실행의 기대 판정은 기록되어 있지 않습니다.'));
    const download=node('a','코드·평가 결과 내려받기 (JSON)');download.href=`/api/submission-download?${new URLSearchParams({problem_id:s.problem_id,submission_id:s.id})}`;download.download=`${s.id}.json`;download.className='record-download';target.append(download);
    const toolbar=node('div','');toolbar.className='case-filter';
    const all=node('button',`전체 결과 (${record.cases.length})`),failed=node('button',`실패한 테스트 (${record.cases.filter(c=>c.verdict!=='AC').length})`);
    toolbar.append(all,failed);target.append(toolbar);
    const caseTarget=node('div',''); target.append(caseTarget);
    const renderCases=(onlyFailed: boolean)=>{
      all.setAttribute('aria-pressed',String(!onlyFailed));failed.setAttribute('aria-pressed',String(onlyFailed));
      const filtered=record.cases.filter(c=>!onlyFailed || c.verdict!=='AC');
      const {wrapper,body}=resultTable(['테스트케이스','판정','실행 시간','메모리','상세']);
      for (const c of filtered) {
        const row=body.insertRow();row.dataset.verdict=c.verdict;row.insertCell().textContent=c.number ? `${c.suite} #${c.number}` : '컴파일';row.insertCell().append(verdictBadge(c.verdict));row.insertCell().textContent=`${c.time_ms.toLocaleString()} ms`;row.insertCell().textContent=memoryLabel(c.memory_kb);
        const button=node('button','자세히 보기');button.setAttribute('aria-label',`${c.number ? `${c.suite} #${c.number}` : '컴파일'} 자세히 보기`);button.onclick=()=>showCase(s,c);row.insertCell().append(button);
      }
      caseTarget.replaceChildren(wrapper);
      if (!filtered.length) caseTarget.append(node('p',record.verdict==='JUDGING' ? '채점 중입니다. 완료되면 결과가 자동으로 표시됩니다.' : onlyFailed ? '실패한 테스트가 없습니다.' : '테스트케이스 결과가 없습니다.'));
    };
    all.onclick=()=>renderCases(false);failed.onclick=()=>renderCases(true);renderCases(false);
    const code=document.createElement('details');code.className='submitted-code';code.append(node('summary',`제출 코드 · ${s.language} · ${sizeLabel(new TextEncoder().encode(record.source).length)}`));code.addEventListener('toggle',()=>{if(code.open&&code.children.length===1)code.append(node('pre',record.source));});target.append(code);
  } catch(e) {if(version===detailVersion)target.textContent=errorMessage(e);}
}
function submissionsTable(items: SubmissionSummary[], target: HTMLElement, showProblem = false) {
  const columns=['제출','판정','테스트','사용량','언어','코드 크기','제출 시각','상세'];
  const {wrapper,body}=resultTable(showProblem ? ['문제',...columns] : columns);
  for (const s of items) {
    const row=body.insertRow();row.dataset.submissionId=s.id;
    if(showProblem) {const cell=row.insertCell();const open=node('button',`${s.problem_name} (${s.problem_id})`);open.onclick=()=>void action(async()=>{await selectProblem(s.problem_id);$('history-dialog').close();});cell.append(open);}
    const id=node('code',s.id.slice(0,8));id.title=s.id;row.insertCell().append(id);
    row.insertCell().append(verdictBadge(s.verdict));row.insertCell().textContent=s.case_count ? `${s.passed_count ?? 0} / ${s.case_count} 통과` : '—';
    const usage=node('div','');usage.className='submission-usage';usage.append(node('span',s.time_ms==null ? '시간: —' : `${s.time_ms.toLocaleString()} ms`),node('small',memoryLabel(s.memory_kb)));row.insertCell().append(usage);
    row.insertCell().textContent=s.language;row.insertCell().textContent=sizeLabel(s.source_bytes);row.insertCell().append(submissionTime(s.created_at));
    const button=node('button','결과 보기');button.className='view-submission';button.dataset.submissionId=s.id;button.setAttribute('aria-label',`제출 ${s.id.slice(0,8)} 결과 보기`);button.onclick=()=>void showSubmission(s,target);const actions=row.insertCell(); actions.append(button);
    if (s.verdict !== 'JUDGING') {
      const more=document.createElement('details');more.className='submission-more';more.append(node('summary','기록 관리'));
      const move=node('button','채점 준비로 이동');
      move.onclick=()=>void action(async()=>{ await api('submission_classify',{problem_id:s.problem_id,submission_id:s.id,purpose:'preparation'}); if (selected) await renderSubmissions(); if ($('history-dialog').open) await renderHistory(); });
      more.append(move);actions.append(more);
    }
  }
  return wrapper;
}
async function renderSubmissions(polling = false) {
  const problemId = requireSelected(), version=++submissionVersion;
  const result = await api('submissions_page', {problem_id:problemId, verdict:$('results-verdict').value, language:$('results-language').value, offset:submissionOffset, limit:20});
  if (selected !== problemId || version!==submissionVersion) return;
  if (submissionOffset>0 && submissionOffset>=result.total) {submissionOffset=Math.max(0,Math.floor((result.total-1)/20)*20);return renderSubmissions(polling);} 
  const target = $('submissions'); target.replaceChildren();
  target.append(node('p', `제출 ${result.total.toLocaleString()}건 · 최신순 · 사용량은 테스트별 최댓값`));
  if (!polling) { ++detailVersion; $('submission-detail').hidden = true; }
  target.append(submissionsTable(result.items,$('submission-detail')));
  if (polling && !$('submission-detail').hidden && $('submission-detail').querySelector('[data-verdict="JUDGING"]')) {
    const current = result.items.find(s => s.id === $('submission-detail').dataset.submissionId);
    if (current && current.verdict !== 'JUDGING') await showSubmission(current, $('submission-detail'));
  }
  if (!result.total) target.append(node('p', $('results-verdict').value || $('results-language').value ? '조건에 맞는 제출이 없습니다. 필터를 초기화해 보세요.' : '아직 제출한 풀이가 없습니다. 문제풀이 탭에서 코드를 제출하세요.')); 
  if (result.total > 20) {
    const pagination = node('div',''); pagination.className = 'toolbar';
    for (const [label, offset, disabled] of [['이전 제출',submissionOffset-20,submissionOffset===0],['다음 제출',submissionOffset+20,submissionOffset+20>=result.total]] as const) {
      const button = node('button',label); button.disabled = disabled;
      button.onclick = () => void action(async () => {submissionOffset = offset; await renderSubmissions();}); pagination.append(button);
    }
    pagination.append(node('span',`${Math.floor(submissionOffset/20)+1} / ${Math.ceil(result.total/20)}`));
    target.append(pagination);
  }
}
let historyOffset = 0, historyTotal = 0, historyVersion = 0;
function updateHistoryPagination() {
  ($('history-previous') as HTMLButtonElement).disabled = historyOffset === 0;
  ($('history-next') as HTMLButtonElement).disabled = historyOffset+20 >= historyTotal;
}
async function renderHistory(polling = false) {
  const version = ++historyVersion;
  $('history-status').textContent = '제출 기록을 불러오는 중입니다.';
  const result = await api('submissions_page', {query:$('history-query').value, verdict:$('history-verdict').value, language:$('history-language').value, offset:historyOffset, limit:20}).catch(e => { $('history-status').textContent = errorMessage(e); throw e; });
  if (version !== historyVersion) return;
  if (polling && !$('history-detail').hidden && $('history-detail').querySelector('[data-verdict="JUDGING"]')) {
    const current = result.items.find(s => s.id === $('history-detail').dataset.submissionId);
    if (current && current.verdict !== 'JUDGING') await showSubmission(current, $('history-detail'));
  }
  historyTotal = result.total;
  $('history-status').textContent = `${result.total.toLocaleString()}건 · 최신 제출부터 표시`;
  if (!polling) { ++detailVersion; $('history-detail').hidden=true; }
  $('history-results').replaceChildren(submissionsTable(result.items,$('history-detail'),true));
  if (!result.total) $('history-results').append(node('p','조건에 맞는 제출 기록이 없습니다.'));
  $('history-page').textContent = `${Math.floor(historyOffset/20)+1} / ${Math.max(1,Math.ceil(result.total/20))}`;
  updateHistoryPagination();
}
bind('open-history','click', async () => { $('history-dialog').showModal(); historyOffset = 0; await renderHistory(); });
$('close-history').onclick = () => $('history-dialog').close();
bind('history-search','submit', async () => { historyOffset = 0; await renderHistory(); });
bind('results-filter','submit',async()=>{submissionOffset=0;await renderSubmissions();});
bind('results-refresh','click',()=>renderSubmissions());
bind('results-reset','click',async()=>{$('results-filter').reset();submissionOffset=0;await renderSubmissions();});
bind('history-refresh','click', async () => { historyOffset = 0; await renderHistory(); });
bind('history-previous','click', async () => { if (historyOffset > 0) { historyOffset -= 20; await renderHistory(); } });
bind('history-next','click', async () => { if (historyOffset+20 < historyTotal) { historyOffset += 20; await renderHistory(); } });
async function selectProblem(id: string) {
  await withLoading(`${id} · 문제를 불러오는 중입니다`, problemLoadSteps + 2, async step => {
  step('현재 풀이의 초안을 저장하는 중입니다.');
  await saveDraft();
  step('선택한 문제의 풀이 초안과 언어를 복구하는 중입니다.');
  const nextLanguage = preferredLanguage(id);
  const restored = await readDraft(id, nextLanguage);
  selected = id; ++submissionVersion; $('results-filter').reset(); currentProblem = null; setupCache = null; ++setupVersion; preparationOffset = 0; $('preparation-detail').hidden = true; $('answer-edit-title').textContent = '새 풀이 코드'; showTab('solve'); submissionOffset = 0; language = nextLanguage; $('problem-picker').close();
  $('language').value = language;
  restoreDraft(restored);
  history.replaceState(null, '', `#${encodeURIComponent(id)}`);
  $('empty').hidden = true; $('problem').hidden = false;
  $('answer-form').reset(); $('answer-language').value = language; answerEditor.setValue(''); setEditorLanguage(answerEditor, language);
  await renderProblem(step); editor.layout();
  });
}
function renderSetupOverview(p: Problem) {
  const target = $('setup-overview'); target.replaceChildren();
  for (const [value, label] of [[p.ready ? '채점 가능' : '준비 필요', '정해·입력·기대 출력 상태'], [p.manifest.main_solution ? '등록됨' : '미등록', '기준 정해'], [`${p.tests.length}개`, `${p.manifest.suites.length}개 테스트 셋`], [p.manifest.validator ? '설정됨' : '미설정', '입력 검증기']]) {
    const card=node('div',''); card.className='setup-metric'; card.append(node('strong',value),node('span',label)); target.append(card);
  }
}
let modelTooltip: HTMLElement | null = null;
let modelTooltipSequence = 0;
function modelSymbols(item?: ApiResults['artifact_models'][number]): HTMLElement {
  const symbols = node('span', ''); symbols.className = 'model-symbols';
  const reviews = item?.trust?.reviews || [];
  const status = reviews.some(r=>r.status==='invalid') ? 'invalid' : reviews.some(r=>r.status==='error') ? 'error' : reviews.some(r=>r.status==='uncertain') ? 'uncertain' : reviews.length ? 'valid' : 'unknown';
  const statuses: Record<string,string> = {valid:'유효',invalid:'부적합',uncertain:'불확실',error:'실패'};
  const details = reviews.map(r=>`${statuses[r.status] || r.status} · ${r.response_model || (r.requested_model ? `${r.requested_model} (요청 모델)` : '모델 기록 없음')}\n제공자: ${r.provider}\n요청 모델: ${r.requested_model || '기록 없음'}\n응답 모델: ${r.response_model || '기록 없음'}\n${new Date(r.created_at*1000).toLocaleString()}\n${r.rationale}`).join('\n\n');
  for (const [symbol,label,info,state] of [
    ['✦','생성 모델',`생성 모델: ${item?.generated_by_model || '기록 없음'}\n저장 시 제공된 정보입니다.`,item?.generated_by_model ? 'recorded' : 'unknown'],
    [status==='valid' ? '✓' : status==='invalid' ? '×' : status==='error' ? '!' : '?','AI 검정',`현재 AI 검정: ${reviews.length}건\n${details || '현재 내용의 검정 기록이 없습니다.'}${item?.trust?.historical_count ? `\n\n이전 내용의 검정 ${item.trust.historical_count}건 (현재 결과에서 제외)` : ''}`,status]
  ]) {
    const button=node('button',symbol); button.type='button'; button.className='model-symbol'; button.dataset.state=state;
    button.setAttribute('aria-label', `${label}: ${label==='생성 모델' ? item?.generated_by_model || '기록 없음' : statuses[status] || '현재 기록 없음'}`);
    const hide=()=>{modelTooltip?.remove();modelTooltip=null;button.removeAttribute('aria-describedby');};
    const show=()=>{
      hide(); const tooltip=node('div',info); tooltip.className='model-tooltip'; tooltip.id=`model-tooltip-${++modelTooltipSequence}`; tooltip.setAttribute('role','tooltip');
      document.body.append(tooltip);modelTooltip=tooltip;button.setAttribute('aria-describedby',tooltip.id);
      const rect=button.getBoundingClientRect(); tooltip.style.left=`${Math.max(8,Math.min(rect.left,window.innerWidth-tooltip.offsetWidth-8))}px`;
      tooltip.style.top=`${Math.max(8,rect.bottom+8+tooltip.offsetHeight>window.innerHeight ? rect.top-tooltip.offsetHeight-8 : rect.bottom+8)}px`;
    };
    button.onmouseenter=show;button.onmouseleave=hide;button.onfocus=show;button.onblur=hide;button.onclick=show;button.onkeydown=e=>{if(e.key==='Escape')hide();};symbols.append(button);
  }
  return symbols;
}
window.addEventListener('scroll',()=>{modelTooltip?.remove();modelTooltip=null;},true);
function renderTestTable(p: Problem, modelItems: ApiResults['artifact_models'] = []) {
  const target=$('test-archives'); target.replaceChildren();
  for (const suite of p.manifest.suites) {
    const card=node('section',''); card.className='setup-card';
    const tests=p.tests.filter(t=>t.suite===suite);
    card.append(node('h3',`${suite} · ${tests.length}개 테스트`));
    const download=node('button','테스트 셋 ZIP 내보내기');
    download.onclick=()=>void action(()=>downloadArchive({kind:'suite',problem_id:p.manifest.id,suite})); card.append(download);
    const {wrapper,body}=resultTable(['번호','입력 검증','기대 출력','내보내기']);
    for (const test of tests) {
      const row=body.insertRow(); const identity = row.insertCell(); identity.append(node('span', `#${test.number}`), modelSymbols(modelItems.find(item=>item.target===`test:${test.suite}:${test.number}`)));
      row.insertCell().textContent=test.validation_status || (p.manifest.validator ? '미검증' : '검증기 없음');
      row.insertCell().textContent=test.generation_error || (p.ready ? '현재 내용으로 생성됨' : test.generation ? '재생성 여부 확인 필요' : '생성 필요');
      const button=node('button','ZIP'); button.onclick=()=>void action(()=>downloadArchive({kind:'test',problem_id:p.manifest.id,suite,number:String(test.number)})); row.insertCell().append(button);
    }
    card.append(wrapper); if (!tests.length) card.append(node('p','아직 테스트가 없습니다. 아래에서 입력을 추가하세요.')); target.append(card);
  }
}
function showSetupSection(name: string) {
  for (const button of document.querySelectorAll<HTMLButtonElement>('[data-setup]')) {
    const active=button.dataset.setup===name; button.setAttribute('aria-pressed',String(active)); $(`setup-${button.dataset.setup}`).hidden=!active;
  }
  if (name==='solutions') answerEditor.layout();
}
for (const button of document.querySelectorAll<HTMLButtonElement>('[data-setup]')) button.onclick=()=>showSetupSection(button.dataset.setup!);
function editAnswer(answer?: Answer) {
  const form=$('answer-form'); form.reset();
  field(form,'answer_id').value=answer?.id || '';
  field(form,'language').value=answer?.language || language;
  field(form,'expected').value=answer?.expected || 'AC';
  field(form,'generated_by_model').value=answer?.generated_by_model || '';
  (field(form,'main') as HTMLInputElement).checked=answer ? currentProblem?.manifest.main_solution===answer.id : !currentProblem?.manifest.main_solution;
  $('answer-edit-title').textContent=answer ? `풀이 코드 수정 · ${answer.id.slice(0,8)} · 버전 ${answer.revision}` : '새 풀이 코드';
  answerEditor.setValue(answer?.source || ''); setEditorLanguage(answerEditor,field(form,'language').value); showSetupSection('solutions'); answerEditor.layout();
}
$('answer-new').onclick=()=>editAnswer();
$('answer-expected').addEventListener('change',()=>{if(field($('answer-form'),'expected').value!=='AC') (field($('answer-form'),'main') as HTMLInputElement).checked=false;});
field($('answer-form'),'main').addEventListener('change',()=>{if((field($('answer-form'),'main') as HTMLInputElement).checked) field($('answer-form'),'expected').value='AC';});
function expectation(expected: string | null, actual: string) {
  const text=actual==='JUDGING' ? '검증 중' : !expected ? '기대 판정 미기록' : expected===actual ? '기대 판정 일치' : '기대 판정 불일치';
  const label=node('span',text); label.className='setup-result'; label.dataset.matched=actual==='JUDGING'||!expected ? 'unknown' : String(expected===actual); return label;
}
async function renderSetup(polling = false) {
  const id=requireSelected(), version=++setupVersion;
  $('setup-status').textContent='풀이 코드, 검증 실행과 검정 기록을 불러오는 중입니다.';
  const cache=polling && setupCache?.id===id ? setupCache : null;
  const [answers, runs, validators, modelItems] = await Promise.all([
    cache ? Promise.resolve(cache.answers) : api('answers_list',{problem_id:id}), api('submissions_page',{problem_id:id,purpose:'preparation',offset:preparationOffset,limit:20}),
    cache ? Promise.resolve(cache.validators) : api('validator_runs_list',{problem_id:id}), cache ? Promise.resolve(cache.modelItems) : api('artifact_models',{problem_id:id})
  ]);
  if (version!==setupVersion || selected!==id) return;
  const p=currentProblem;
  if (!p) return;
  setupCache={id,answers,validators,modelItems};
  renderTestTable(p, modelItems);
  const answerTarget=$('setup-answers'); answerTarget.replaceChildren();
  const table=resultTable(['코드','역할','언어','기대 판정','작업']);
  for (const answer of answers) {
    const row=table.body.insertRow(); const identity = row.insertCell(); identity.append(node('span', `${answer.id.slice(0,8)} · 버전 ${answer.revision}`), modelSymbols(modelItems.find(item=>item.target===`answer:${answer.id}`)));
    row.insertCell().textContent=p.manifest.main_solution===answer.id ? '기준 정해' : p.manifest.validator===answer.id ? '입력 검증기' : '검증용 코드';
    row.insertCell().textContent=answer.language; row.insertCell().append(verdictBadge(answer.expected));
    const actions=row.insertCell(), edit=node('button','수정'); edit.onclick=()=>editAnswer(answer); actions.append(edit);
    if (p.manifest.validator!==answer.id) {
      const run=node('button','판정 검증'); run.disabled=!p.ready;
      run.onclick=()=>void action(async()=>{ await api('test_run',{problem_id:id,answer_id:answer.id,async:true}); preparationOffset=0; showSetupSection('verification'); await renderSetup(); }); actions.append(run);
    }
    if (p.manifest.main_solution!==answer.id && p.manifest.validator!==answer.id) {
      const validator=node('button','입력 검증기로 지정'); validator.onclick=()=>void action(async()=>{await api('settings_update',{problem_id:id,validator:answer.id}); await renderProblem();}); actions.append(validator);
    }
  }
  answerTarget.append(table.wrapper); if (!answers.length) answerTarget.append(node('p','등록된 코드가 없습니다. 아래에서 기준 정해를 먼저 저장하세요.'));
  const target=$('preparation-runs'); target.replaceChildren(node('p',`검증 실행 ${runs.total}건 · 최신순`));
  const result=resultTable(['검증 코드','실행 시각','기대 판정','실제 판정','검증 결과','작업']);
  for (const run of runs.items) {
    const row=result.body.insertRow(); row.insertCell().textContent=run.preparation?.answer_id ? `${run.preparation.answer_id.slice(0,8)} · 버전 ${run.preparation.revision}${answers.find(a=>a.id===run.preparation?.answer_id)?.revision!==run.preparation.revision ? ' · 이전 버전' : ''}` : run.preparation?.name || '이전 검증 실행'; row.insertCell().textContent=new Date(run.created_at*1000).toLocaleString();
    row.insertCell().textContent=run.preparation?.expected || '미기록'; row.insertCell().append(verdictBadge(run.verdict)); row.insertCell().append(expectation(run.preparation?.expected || null,run.verdict));
    const actions=row.insertCell(), view=node('button','결과 보기'); view.onclick=()=>void showSubmission(run,$('preparation-detail')); actions.append(view);
    if (run.verdict!=='JUDGING') { const move=node('button','풀이 제출로 이동'); move.onclick=()=>void action(async()=>{await api('submission_classify',{problem_id:id,submission_id:run.id,purpose:'solve'}); await renderSetup(); await renderSubmissions();}); actions.append(move); }
  }
  target.append(result.wrapper); if (!runs.total) target.append(node('p','풀이 코드에서 판정 검증을 실행하면 기록이 표시됩니다.'));
  if (runs.total>20) {
    const nav=node('div',''); nav.className='toolbar';
    for (const [label,offset,disabled] of [['이전',preparationOffset-20,preparationOffset===0],['다음',preparationOffset+20,preparationOffset+20>=runs.total]] as const) {
      const button=node('button',label); button.disabled=disabled; button.onclick=()=>void action(async()=>{preparationOffset=offset; await renderSetup();});nav.append(button);
    }
    nav.append(node('span',`${Math.floor(preparationOffset/20)+1} / ${Math.ceil(runs.total/20)}`)); target.append(nav);
  }
  if (!polling) $('preparation-detail').hidden=true;
  else if (!$('preparation-detail').hidden && $('preparation-detail').querySelector('[data-verdict="JUDGING"]')) {
    const updated=runs.items.find(r=>r.id===$('preparation-detail').dataset.submissionId); if (updated && updated.verdict!=='JUDGING') await showSubmission(updated,$('preparation-detail'));
  }
  const validatorTarget=$('validator-runs'); validatorTarget.replaceChildren();
  const validatorTable=resultTable(['실행 시각','검증기 버전','기대 결과','실제 결과','검증 결과']);
  for (const run of validators.sort((a,b)=>b.created_at-a.created_at).slice(0,20)) {
    const row=validatorTable.body.insertRow(); row.insertCell().textContent=new Date(run.created_at*1000).toLocaleString(); row.insertCell().textContent=`${run.validator_id.slice(0,8)} · 버전 ${run.revision}`;
    row.insertCell().textContent=run.expect_valid ? '통과' : '거부';row.insertCell().textContent=run.accepted ? '통과' : '거부';row.insertCell().append(expectation(run.expect_valid ? '통과' : '거부',run.accepted ? '통과' : '거부'));
  }
  validatorTarget.append(node('p',`입력 검증 ${validators.length}건 · 최근 20건 표시`),validatorTable.wrapper);
  ($('validator-form').querySelector('button') as HTMLButtonElement).disabled=!p.manifest.validator;
  $('setup-status').textContent=p.ready ? '채점 준비가 완료되었습니다. 검증용 풀이로 테스트의 탐지 능력을 확인하세요.' : '기준 정해와 테스트를 등록하고 기대 출력을 생성하세요.';
}
bind('setup-refresh','click',()=>renderProblem());
bind('validator-form','submit',async()=>{const form=$('validator-form');await api('validator_run',{problem_id:requireSelected(),input:field(form,'input').value,expect_valid:field(form,'expect_valid').value==='true'});await renderSetup();});
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
  activateSource(source); backup(); await saveDraft();
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
  try { await api('submit', {...snapshot(), summary_only:true, async:true}); $('results-filter').reset(); submissionOffset = 0; }
  catch (e) { await renderSubmissions().catch(() => {}); throw new Error(`${errorMessage(e)} 제출 기록을 확인한 뒤 다시 시도하세요.`); }
  await renderSubmissions(); showTab('results'); ($('submissions').querySelector('.view-submission') as HTMLButtonElement | null)?.click();
});
bind('workspace-open','submit', async () => { const path = field($('workspace-open'), 'path').value; const result = await api('workspace_read',{path}); await openSource(path, result.content); });
bind('refresh','click', () => withLoading('화면을 새로 불러오는 중입니다', selected ? problemLoadSteps + 2 : 2, async step => {
  step('현재 풀이의 초안을 저장하는 중입니다.');
  await saveDraft();
  step('문제 목록을 다시 불러오는 중입니다.');
  await refresh();
  if (selected) await renderProblem(step);
}));
let searchTimer: ReturnType<typeof setTimeout>;
$('filter').addEventListener('input', () => { ++searchVersion; clearTimeout(searchTimer); page = 0; searchTimer = setTimeout(() => void renderList().catch(e => notice(errorMessage(e),true)), 250); });
bind('create','submit', async () => { const args = {problem_id:field($('create'), 'problem_id').value, name:field($('create'), 'name').value}; await api('problem_create',args); await refresh(); await selectProblem(args.problem_id); showTab('setup'); });
bind('import','submit', async () => { const args = {problem_id:field($('import'), 'problem_id').value, path:field($('import'), 'path').value}; const result = await api('problem_import',args); await refresh(); await selectProblem(args.problem_id); if (result.warnings?.length) $('readiness').textContent += ` · ${result.warnings.join(' · ')}`; });
bind('statement-form','submit', async () => { await api('statement_set',{problem_id:requireSelected(), section:'legend', markdown:field($('statement-form'), 'markdown').value}); await renderProblem(); });
$('answer-language').addEventListener('change', () => setEditorLanguage(answerEditor, $('answer-language').value));
bind('answer-form','submit', async () => {
  if (!answerEditor.getValue().trim()) throw new Error('정해 코드를 입력하세요.');
  const form = $('answer-form');
  const result = await api('answer_save',{problem_id:requireSelected(),language:field(form, 'language').value,source:answerEditor.getValue(),expected:field(form, 'expected').value, main:(field(form, 'main') as HTMLInputElement).checked, ...(field(form,'answer_id').value ? {answer_id:field(form,'answer_id').value} : {}),generated_by_model:field(form, 'generated_by_model').value});
  await renderProblem(); editAnswer(result.answer); if (result.regeneration?.error) throw new Error(result.regeneration.error);
});
bind('test-form','submit', async () => { const form = $('test-form'); await api('test_put',{problem_id:requireSelected(),suite:field(form, 'suite').value,number:Number(field(form, 'number').value),input:field(form, 'input').value,generated_by_model:field(form, 'generated_by_model').value}); await renderProblem(); });
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
  await withLoading('앱을 준비하는 중입니다', 3, async step => {
  step('연결된 작업 폴더와 지원 언어를 확인하는 중입니다.');
  capabilities = await api('capabilities');
  $('workspace').textContent = capabilities.workspace ? `워크스페이스 · ${capabilities.workspace}` : '즉석 작성 · 폴더 연결 없음';
  $('import-panel').hidden = !capabilities.workspace;
  $('workspace-files').hidden = !capabilities.workspace;
  for (const id of Object.keys(capabilities.languages)) for (const select of [$('language'),$('answer-language')]) { const option = node('option',id); option.value=id; select.append(option); }
  for (const id of Object.keys(capabilities.languages)) for (const select of [$('history-language'),$('results-language')]) { const option = node('option',id); option.value = id; select.append(option); }
  language = capabilities.languages.python ? 'python' : Object.keys(capabilities.languages)[0];
  $('language').value = language;
  step('문제 목록을 불러오는 중입니다.');
  await refresh();
  step('작업 폴더의 파일 목록을 확인하는 중입니다.');
  if (capabilities.workspace) {
    const entries = await api('workspace_list');
    for (const file of entries.filter(f => !f.directory)) { const button=node('button',file.path); button.onclick=()=>void action(async()=>{ if (!selected) throw new Error('먼저 문제를 선택하세요.'); const r=await api('workspace_read',{path:file.path}); await openSource(file.path,r.content); }); $('files').append(button); }
  }
  });
  const id = decodeURIComponent(location.hash.slice(1));
  if (id) await selectProblem(id);
});

// Refresh pending verdicts without keeping the submission request open.
let pollingSubmissions = false;
window.setInterval(async () => {
  if (pollingSubmissions || document.hidden) return;
  pollingSubmissions = true;
  try {
    if (selected && !$('panel-setup').hidden && $('preparation-runs').querySelector('[data-verdict="JUDGING"]')) await renderSetup(true);
    if (selected && $('submissions').querySelector('[data-verdict="JUDGING"]')) await renderSubmissions(true);
    if ($('history-dialog').open && $('history-results').querySelector('[data-verdict="JUDGING"]')) await renderHistory(true);
  } catch (e) { console.error(e); }
  finally { pollingSubmissions = false; }
}, 1500);
