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
    pub id: String,
    pub revision: u64,
    pub language: String,
    pub source: String,
    pub expected: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestMeta {
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
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseResult {
    pub suite: String,
    pub number: u64,
    pub result: RunResult,
    pub checker_message: String,
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub expected_output: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Submission {
    pub id: String,
    pub problem_id: String,
    pub language: String,
    pub source: String,
    pub created_at: u64,
    pub generation: String,
    pub verdict: String,
    pub cases: Vec<CaseResult>,
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
