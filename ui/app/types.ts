// Fields consumed by the installed UI from the Rust HTTP API.
export interface Manifest { suites: string[]; id: string; name: string; time_limit_ms: number; memory_limit_mb: number; main_solution: string | null; validator: string | null; checker: string }
export interface RunResult { verdict: string; time_ms: number; memory_kb: number; stdout: string; stderr: string }
export interface CaseResult { suite: string; number: number; result: RunResult; checker_message: string; input?: string; expected_output?: string }
export interface Submission { id: string; problem_id: string; language: string; source: string; created_at: number; generation: string; verdict: string; cases: CaseResult[] }
export interface TestMeta { suite: string; number: number; generation: string | null; generation_error: string | null; validation_status: string | null }
export interface Capabilities { storage_root: string; workspace: string | null; languages: Record<string, {filename: string; compile: string[]; run: string[]}> }
export interface Draft { problem_id: string; language: string; source: string }
export interface Problem { manifest: Manifest; ready: boolean; statements: Record<string, {markdown: string; html: string}>; examples: {number: number; input: string; output: string}[]; tests: TestMeta[] }
export interface Answer { id: string; language: string; source: string; expected: string; revision: number; generated_by_model: string | null }
export interface ValidatorRun { id: string; created_at: number; expect_valid: boolean; accepted: boolean; matched: boolean; revision: number; validator_id: string; input: string; result: RunResult }
export interface ApiResults {
  answers_list: Answer[];
  test_run: {submission: Submission; expected: string; matched: boolean | null};
  settings_update: Manifest;
  validator_run: ValidatorRun;
  validator_runs_list: ValidatorRun[];
  submission_classify: SubmissionSummary;
  artifact_models: {label: string; target: string; generated_by_model: string | null; trust: {historical_count: number; reviews: {provider: string; requested_model: string | null; response_model: string | null; status: string; rationale: string; created_at: number}[]} | null}[];
  capabilities: Capabilities;
  problems_list: Manifest[];
  problems_search: {items: {id: string; name: string}[]; total: number};
  problem_get: Problem;
  submissions_list: Submission[];
  submissions_page: {items: SubmissionSummary[]; total: number};
  submission_preview: SubmissionPreview;
  submission_data_preview: TextPreview & {path: string; exists: boolean; current_test: boolean};
  draft_get: Draft | null;
  draft_save: unknown;
  workspace_read: {content: string};
  workspace_list: {path: string; directory: boolean}[];
  problem_create: unknown;
  problem_import: {warnings?: string[]};
  statement_set: unknown;
  answer_save: {answer: Answer; regeneration?: {error?: string} | null};
  test_put: unknown;
  submit: unknown;
  regenerate: unknown;
}

export interface TextPreview {text: string; bytes: number; truncated: boolean}
export interface SubmissionPreview {verdict: string; source: string; case_count: number; cases: {index: number; suite: string; number: number; verdict: string; time_ms: number; memory_kb: number; exit_code: number | null}[]}

export type SubmissionSummary = Pick<Submission, 'id' | 'problem_id' | 'language' | 'created_at' | 'verdict'> & {time_ms: number | null; memory_kb: number | null; source_bytes: number | null; case_count: number | null; passed_count: number | null; problem_name: string; purpose: 'solve' | 'preparation'; preparation: {name: string; expected: string | null; answer_id: string | null; revision: number | null} | null};
