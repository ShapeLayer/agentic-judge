use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub storage_root: PathBuf,
    pub runtime_root: PathBuf,
    pub workspace_root: Option<PathBuf>,
    pub container: bool,
    pub container_command: String,
    pub image: String,
    pub languages: BTreeMap<String, Language>,
    pub providers: BTreeMap<String, Provider>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            storage_root: "data".into(),
            runtime_root: "run".into(),
            workspace_root: None,
            container: true,
            container_command: "podman".into(),
            image: concat!(
                "localhost/agentic-judge-runtime:",
                env!("CARGO_PKG_VERSION")
            )
            .into(),
            languages: default_languages(),
            providers: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Language {
    pub filename: String,
    pub compile: Vec<String>,
    pub run: Vec<String>,
}
fn default_languages() -> BTreeMap<String, Language> {
    [
        (
            "cpp",
            "main.cpp",
            vec![
                "g++",
                "-std=c++20",
                "-O2",
                "-I.",
                "main.cpp",
                "-o",
                "program",
            ],
            vec!["./program"],
        ),
        (
            "c",
            "main.c",
            vec!["gcc", "-std=c17", "-O2", "main.c", "-lm", "-o", "program"],
            vec!["./program"],
        ),
        (
            "rust",
            "main.rs",
            vec!["rustc", "-O", "main.rs", "-o", "program"],
            vec!["./program"],
        ),
        ("python", "main.py", vec![], vec!["python3", "main.py"]),
        ("javascript", "main.js", vec![], vec!["node", "main.js"]),
        (
            "typescript",
            "main.ts",
            vec!["tsc", "main.ts", "--target", "es2020"],
            vec!["node", "main.js"],
        ),
        (
            "java",
            "Main.java",
            vec!["javac", "Main.java"],
            vec!["java", "-Xmx128m", "Main"],
        ),
        (
            "go",
            "main.go",
            vec!["go", "build", "-o", "program", "main.go"],
            vec!["./program"],
        ),
        ("ruby", "main.rb", vec![], vec!["ruby", "main.rb"]),
        ("php", "main.php", vec![], vec!["php", "main.php"]),
        ("bash", "main.sh", vec![], vec!["bash", "main.sh"]),
    ]
    .into_iter()
    .map(|(id, filename, compile, run)| {
        (
            id.into(),
            Language {
                filename: filename.into(),
                compile: compile.into_iter().map(String::from).collect(),
                run: run.into_iter().map(String::from).collect(),
            },
        )
    })
    .collect()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provider {
    pub kind: String,
    pub endpoint: String,
    pub model: String,
    pub api_key_env: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub time_limit_ms: u64,
    pub memory_limit_mb: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    pub example_inputs: usize,
    pub example_outputs: usize,
    pub checker: String,
    pub suites: Vec<String>,
    pub main_solution: Option<String>,
    pub validator: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Answer {
    /// Caller-declared generation model; absent for legacy or unknown sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by_model: Option<String>,
    pub id: String,
    pub revision: u64,
    pub language: String,
    pub source: String,
    pub expected: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by_model: Option<String>,
    pub suite: String,
    pub number: u64,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub generation: Option<String>,
    pub validator_hash: Option<String>,
    #[serde(default)]
    pub validation_status: Option<String>,
    #[serde(default)]
    pub generation_error: Option<String>,
    #[serde(default)]
    pub origin: Option<serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub id: String,
    pub provider: String,
    /// Captured from configuration before the request, including failed attempts.
    #[serde(default)]
    pub requested_model: Option<String>,
    /// Provider response metadata, never the model's self-reported identity.
    #[serde(default)]
    pub response_model: Option<String>,
    pub target: String,
    pub fingerprint: String,
    pub status: String,
    pub rationale: String,
    pub suggested_direction: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunResult {
    pub verdict: String,
    pub time_ms: u64,
    pub memory_kb: u64,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub exit_code: Option<i32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseResult {
    pub suite: String,
    pub number: u64,
    #[serde(serialize_with = "serialize_evaluation")]
    pub result: RunResult,
    #[serde(default, skip_serializing)]
    pub checker_message: String,
    #[serde(default, skip_serializing)]
    pub input: String,
    #[serde(default, skip_serializing)]
    pub expected_output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output_hash: Option<String>,
}
fn serialize_evaluation<S: serde::Serializer>(
    result: &RunResult,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut fields = serializer.serialize_struct("EvaluationResult", 4)?;
    fields.serialize_field("verdict", &result.verdict)?;
    fields.serialize_field("time_ms", &result.time_ms)?;
    fields.serialize_field("memory_kb", &result.memory_kb)?;
    fields.serialize_field("exit_code", &result.exit_code)?;
    fields.end()
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionPurpose {
    #[default]
    Solve,
    Preparation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparationRun {
    pub name: String,
    pub expected: Option<String>,
    pub answer_id: Option<String>,
    pub revision: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    #[serde(default)]
    pub purpose: SubmissionPurpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation: Option<PreparationRun>,
    #[serde(default)]
    pub format_version: u32,
    pub id: String,
    pub problem_id: String,
    pub language: String,
    pub source: String,
    pub created_at: u64,
    pub generation: String,
    pub verdict: String,
    pub cases: Vec<CaseResult>,
}
/// Lightweight metadata; serde skips source and large case payloads when reading old records.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubmissionSummary {
    #[serde(default)]
    pub time_ms: Option<u64>,
    #[serde(default)]
    pub memory_kb: Option<u64>,
    #[serde(default)]
    pub source_bytes: Option<u64>,
    #[serde(default)]
    pub case_count: Option<u64>,
    #[serde(default)]
    pub passed_count: Option<u64>,
    #[serde(default)]
    pub purpose: SubmissionPurpose,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preparation: Option<PreparationRun>,
    pub id: String,
    pub problem_id: String,
    pub language: String,
    pub created_at: u64,
    pub verdict: String,
}
impl From<&Submission> for SubmissionSummary {
    fn from(s: &Submission) -> Self {
        Self {
            time_ms: s.cases.iter().map(|c| c.result.time_ms).max(),
            memory_kb: s.cases.iter().map(|c| c.result.memory_kb).max(),
            source_bytes: Some(s.source.len() as u64),
            case_count: Some(s.cases.iter().filter(|c| c.number > 0).count() as u64),
            passed_count: Some(
                s.cases
                    .iter()
                    .filter(|c| c.number > 0 && c.result.verdict == "AC")
                    .count() as u64,
            ),
            purpose: s.purpose.clone(),
            preparation: s.preparation.clone(),
            id: s.id.clone(),
            problem_id: s.problem_id.clone(),
            language: s.language.clone(),
            created_at: s.created_at,
            verdict: s.verdict.clone(),
        }
    }
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
pub fn hash(bytes: impl AsRef<[u8]>) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
