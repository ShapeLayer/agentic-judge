// Fields consumed by the installed UI from the Rust HTTP API.
export interface Manifest { suites: string[]; id: string; name: string; time_limit_ms: number; memory_limit_mb: number }
export interface RunResult { verdict: string; time_ms: number; memory_kb: number; stdout: string; stderr: string }
export interface CaseResult { suite: string; number: number; result: RunResult; checker_message: string; input?: string; expected_output?: string }
export interface Submission { id: string; problem_id: string; language: string; source: string; created_at: number; generation: string; verdict: string; cases: CaseResult[] }
export interface TestMeta { suite: string; number: number }
export interface Capabilities { storage_root: string; workspace: string | null; languages: Record<string, {filename: string; compile: string[]; run: string[]}> }
export interface Draft { problem_id: string; language: string; source: string }
export interface Problem { manifest: Manifest; ready: boolean; statements: Record<string, {markdown: string; html: string}>; examples: {number: number; input: string; output: string}[]; tests: TestMeta[] }
export interface ApiResults {
  capabilities: Capabilities;
  problems_list: Manifest[];
  problem_get: Problem;
  submissions_list: Submission[];
  draft_get: Draft | null;
  draft_save: unknown;
  workspace_read: {content: string};
  workspace_list: {path: string; directory: boolean}[];
  problem_create: unknown;
  problem_import: {warnings?: string[]};
  statement_set: unknown;
  answer_save: {regeneration?: {error?: string} | null};
  test_put: unknown;
  submit: unknown;
  regenerate: unknown;
}
