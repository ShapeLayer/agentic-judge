use crate::model::*;
use anyhow::{Result, bail, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

type ProblemCatalog = BTreeMap<String, (SystemTime, u64, Manifest)>;

#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
    catalog: Arc<Mutex<ProblemCatalog>>,
}
pub fn identifier(s: &str) -> Result<()> {
    ensure!(
        !s.is_empty()
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "invalid identifier"
    );
    Ok(())
}
pub fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap();
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", uuid()));
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic(path, &serde_json::to_vec_pretty(value)?)
}
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
impl Store {
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        Ok(Self {
            root: root.canonicalize()?,
            catalog: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }
    pub fn problem(&self, id: &str) -> Result<PathBuf> {
        identifier(id)?;
        let p = self.root.join("problems").join(id);
        if p.exists() {
            ensure!(
                p.canonicalize()?.starts_with(&self.root),
                "path outside storage"
            );
        }
        Ok(p)
    }
    pub fn manifest(&self, id: &str) -> Result<Manifest> {
        Ok(toml::from_str(&fs::read_to_string(
            self.problem(id)?.join("manifest.toml"),
        )?)?)
    }
    pub fn save_manifest(&self, m: &Manifest) -> Result<()> {
        atomic(
            &self.problem(&m.id)?.join("manifest.toml"),
            toml::to_string_pretty(m)?.as_bytes(),
        )
    }
    pub fn create(&self, id: &str, name: &str) -> Result<Manifest> {
        let p = self.problem(id)?;
        ensure!(!p.exists(), "problem already exists");
        fs::create_dir_all(p.join("statements"))?;
        fs::create_dir_all(p.join("tests/general"))?;
        for section in ["legend", "input", "output", "notes"] {
            fs::write(p.join(format!("statements/{section}.md")), "")?;
        }
        let m = Manifest {
            id: id.into(),
            name: name.into(),
            time_limit_ms: 2000,
            memory_limit_mb: 256,
            tags: vec![],
            example_inputs: 0,
            example_outputs: 0,
            checker: "tokens".into(),
            suites: vec!["general".into()],
            main_solution: None,
            validator: None,
        };
        self.save_manifest(&m)?;
        Ok(m)
    }
    pub fn list(&self) -> Result<Vec<Manifest>> {
        let p = self.root.join("problems");
        if !p.exists() {
            return Ok(vec![]);
        }
        let mut result = vec![];
        for e in fs::read_dir(p)? {
            let e = e?;
            if e.file_type()?.is_dir() {
                result.push(self.manifest(&e.file_name().to_string_lossy())?);
            }
        }
        result.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(result)
    }
    /// Reuse parsed manifests while checking file stamps, including other sessions' imports.
    pub fn search(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<(String, String)>, usize)> {
        let directory = self.root.join("problems");
        let mut catalog = self
            .catalog
            .lock()
            .map_err(|_| anyhow::anyhow!("problem catalog unavailable"))?;
        let mut seen = std::collections::BTreeSet::new();
        if directory.exists() {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let id = entry.file_name().to_string_lossy().into_owned();
                let metadata = fs::metadata(self.problem(&id)?.join("manifest.toml"))?;
                let stamp = metadata.modified()?;
                let length = metadata.len();
                if !catalog.get(&id).is_some_and(|(old_stamp, old_length, _)| {
                    *old_stamp == stamp && *old_length == length
                }) {
                    catalog.insert(id.clone(), (stamp, length, self.manifest(&id)?));
                }
                seen.insert(id);
            }
        }
        catalog.retain(|id, _| seen.contains(id));
        let query = query.to_lowercase();
        let mut total = 0;
        let mut items = vec![];
        for (_, _, manifest) in catalog.values() {
            if format!(
                "{} {} {}",
                manifest.id,
                manifest.name,
                manifest.tags.join(" ")
            )
            .to_lowercase()
            .contains(&query)
            {
                if total >= offset && items.len() < limit {
                    items.push((manifest.id.clone(), manifest.name.clone()));
                }
                total += 1;
            }
        }
        Ok((items, total))
    }
    pub fn answer(&self, problem: &str, id: &str) -> Result<Answer> {
        identifier(id)?;
        read_json(
            &self
                .problem(problem)?
                .join(format!("answers/{id}/current.json")),
        )
    }
    pub fn save_answer(
        &self,
        problem: &str,
        id: Option<&str>,
        language: &str,
        source: &str,
        expected: &str,
    ) -> Result<Answer> {
        self.save_answer_with_model(problem, id, language, source, expected, None)
    }
    pub fn save_answer_with_model(
        &self,
        problem: &str,
        id: Option<&str>,
        language: &str,
        source: &str,
        expected: &str,
        generated_by_model: Option<String>,
    ) -> Result<Answer> {
        ensure!(
            ["AC", "WA", "RE", "TLE", "MLE"].contains(&expected),
            "invalid expected verdict"
        );
        let id = id.map(String::from).unwrap_or_else(uuid);
        uuid::Uuid::parse_str(&id)?;
        let p = self.problem(problem)?.join(format!("answers/{id}"));
        let old: Option<Answer> = if p.join("current.json").exists() {
            Some(read_json(&p.join("current.json"))?)
        } else {
            None
        };
        if let Some(a) = &old
            && a.source == source
            && a.language == language
            && a.expected == expected
            && a.generated_by_model == generated_by_model
        {
            return Ok(a.clone());
        }
        let a = Answer {
            generated_by_model,
            id,
            revision: old.map_or(1, |a| a.revision + 1),
            language: language.into(),
            source: source.into(),
            expected: expected.into(),
            created_at: now(),
        };
        write_json(&p.join(format!("revisions/{}.json", a.revision)), &a)?;
        write_json(&p.join("current.json"), &a)?;
        Ok(a)
    }
    pub fn tests(&self, id: &str) -> Result<Vec<TestMeta>> {
        let m = self.manifest(id)?;
        let mut result = vec![];
        for suite in m.suites {
            identifier(&suite)?;
            let p = self.problem(id)?.join(format!("tests/{suite}"));
            if !p.exists() {
                continue;
            }
            for e in fs::read_dir(p)? {
                let p = e?.path();
                if p.extension().is_some_and(|s| s == "json") {
                    result.push(read_json::<TestMeta>(&p)?);
                }
            }
        }
        result.sort_by(|a, b| (&a.suite, a.number).cmp(&(&b.suite, b.number)));
        Ok(result)
    }
    pub fn test_path(&self, id: &str, suite: &str, n: u64, ext: &str) -> Result<PathBuf> {
        identifier(suite)?;
        ensure!(n > 0, "test number must be positive");
        Ok(self.problem(id)?.join(format!("tests/{suite}/{n}.{ext}")))
    }
    pub fn put_test(&self, id: &str, suite: &str, n: u64, input: &str) -> Result<TestMeta> {
        let mut m = self.manifest(id)?;
        identifier(suite)?;
        if !m.suites.contains(&suite.to_string()) {
            m.suites.push(suite.into());
            self.save_manifest(&m)?;
        }
        let meta = TestMeta {
            generated_by_model: None,
            suite: suite.into(),
            number: n,
            input_hash: hash(input),
            output_hash: None,
            generation: None,
            validator_hash: None,
            validation_status: None,
            generation_error: None,
            origin: None,
        };
        atomic(&self.test_path(id, suite, n, "in")?, input.as_bytes())?;
        let out = self.test_path(id, suite, n, "out")?;
        if out.exists() {
            fs::remove_file(out)?;
        }
        write_json(&self.test_path(id, suite, n, "json")?, &meta)?;
        Ok(meta)
    }
    pub fn generation(&self, id: &str) -> Result<String> {
        let m = self.manifest(id)?;
        let a = self.answer(
            id,
            m.main_solution
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("main solution missing"))?,
        )?;
        Ok(hash(serde_json::to_vec(&(
            a,
            m.time_limit_ms,
            m.memory_limit_mb,
            m.validator
                .as_ref()
                .map(|v| self.answer(id, v))
                .transpose()?,
        ))?))
    }
    pub fn ready(&self, id: &str) -> Result<bool> {
        let generation = match self.generation(id) {
            Ok(g) => g,
            Err(_) => return Ok(false),
        };
        let tests = self.tests(id)?;
        if tests.is_empty() {
            return Ok(false);
        }
        for t in tests {
            if t.generation.as_ref() != Some(&generation) {
                return Ok(false);
            }
            if hash(fs::read(self.test_path(id, &t.suite, t.number, "in")?)?) != t.input_hash {
                return Ok(false);
            }
            let out = self.test_path(id, &t.suite, t.number, "out")?;
            if !out.exists() || Some(hash(fs::read(out)?)) != t.output_hash {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn submission_path(&self, problem_id: &str, submission_id: &str) -> Result<PathBuf> {
        identifier(submission_id)?;
        let directory = self.problem(problem_id)?.join("submissions");
        let path = directory.join(format!("{submission_id}.json"));
        let problem = self.problem(problem_id)?.canonicalize()?;
        let canonical_directory = directory.canonicalize()?;
        ensure!(
            canonical_directory.starts_with(&problem)
                && path.canonicalize()?.starts_with(&canonical_directory),
            "record outside problem storage"
        );
        Ok(path)
    }
    pub fn persist_case_data(
        &self,
        problem_id: &str,
        submission_id: &str,
        index: usize,
        case: &mut CaseResult,
    ) -> Result<()> {
        identifier(submission_id)?;
        let problem = self.problem(problem_id)?.canonicalize()?;
        let mut directory = problem.clone();
        for component in [
            "submission-data".to_owned(),
            submission_id.to_owned(),
            index.to_string(),
        ] {
            directory.push(component);
            if directory.exists() {
                ensure!(
                    directory.canonicalize()?.starts_with(&problem),
                    "evaluation data outside problem storage"
                );
            } else {
                fs::create_dir(&directory)?;
            }
        }
        for (field, value) in [
            ("stdout", &mut case.result.stdout),
            ("stderr", &mut case.result.stderr),
            ("checker", &mut case.checker_message),
        ] {
            let path = directory.join(format!("{field}.txt"));
            // Migration is idempotent: an already separated output must not be overwritten by an empty field.
            if !value.is_empty() || !path.exists() {
                atomic(&path, value.as_bytes())?;
            }
            *value = String::new();
        }
        if case.number > 0 {
            if case.input_hash.is_none() {
                case.input_hash = Some(hash(case.input.as_bytes()));
            }
            if case.expected_output_hash.is_none() {
                case.expected_output_hash = Some(hash(case.expected_output.as_bytes()));
            }
        }
        case.input = String::new();
        case.expected_output = String::new();
        Ok(())
    }
    pub fn compact_submission(&self, problem_id: &str, submission_id: &str) -> Result<(u64, u64)> {
        let path = self.submission_path(problem_id, submission_id)?;
        let before = fs::metadata(&path)?.len();
        let mut record: Submission = read_json(&path)?;
        if record.format_version >= 2 {
            return Ok((before, before));
        }
        ensure!(
            record.problem_id == problem_id && record.id == submission_id,
            "submission identity mismatch"
        );
        for (index, case) in record.cases.iter_mut().enumerate() {
            self.persist_case_data(problem_id, submission_id, index, case)?;
        }
        record.format_version = 2;
        write_json(&path, &record)?;
        self.save_submission_summary(&SubmissionSummary::from(&record))?;
        Ok((before, fs::metadata(path)?.len()))
    }
    pub fn evaluation_data_preview(
        &self,
        problem_id: &str,
        submission_id: &str,
        index: usize,
        field: &str,
    ) -> Result<serde_json::Value> {
        use std::io::Read;
        let record: Submission = read_json(&self.submission_path(problem_id, submission_id)?)?;
        let case = record
            .cases
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("case index out of range"))?;
        let problem = self.problem(problem_id)?.canonicalize()?;
        let (path, current_test) = match field {
            "input" | "expected_output" => {
                ensure!(case.number > 0, "compile result has no test data");
                (
                    self.test_path(
                        problem_id,
                        &case.suite,
                        case.number,
                        if field == "input" { "in" } else { "out" },
                    )?,
                    true,
                )
            }
            "stdout" | "stderr" | "checker" => (
                problem.join(format!(
                    "submission-data/{submission_id}/{index}/{field}.txt"
                )),
                false,
            ),
            _ => bail!("unknown evaluation data field"),
        };
        if !path.exists() {
            return Ok(
                serde_json::json!({"path":path,"exists":false,"text":"","bytes":0,"truncated":false,"current_test":current_test}),
            );
        }
        let canonical = path.canonicalize()?;
        ensure!(
            canonical.starts_with(&problem),
            "evaluation data outside problem storage"
        );
        let bytes = fs::metadata(&canonical)?.len();
        let mut prefix = vec![];
        fs::File::open(&canonical)?
            .take(4096 * 4)
            .read_to_end(&mut prefix)?;
        let valid = match std::str::from_utf8(&prefix) {
            Ok(value) => value,
            Err(error) => std::str::from_utf8(&prefix[..error.valid_up_to()])?,
        };
        let text = valid.chars().take(4096).collect::<String>();
        let truncated = (text.len() as u64) < bytes;
        Ok(
            serde_json::json!({"path":canonical,"exists":true,"text":text,"bytes":bytes,"truncated":truncated,"current_test":current_test}),
        )
    }
    pub fn save_submission_summary(&self, summary: &SubmissionSummary) -> Result<()> {
        identifier(&summary.id)?;
        write_json(
            &self
                .problem(&summary.problem_id)?
                .join(format!("submission-summaries/{}.json", summary.id)),
            summary,
        )
    }
    pub fn submission_summary_metrics(
        &self,
        summary: SubmissionSummary,
    ) -> Result<SubmissionSummary> {
        if summary.source_bytes.is_some() {
            return Ok(summary);
        }
        // Upgrade only records on the requested page; subsequent reads use the small cache.
        let record: Submission =
            read_json(&self.submission_path(&summary.problem_id, &summary.id)?)?;
        let updated = SubmissionSummary::from(&record);
        self.save_submission_summary(&updated)?;
        Ok(updated)
    }
    pub fn submission_summaries(&self, id: &str) -> Result<Vec<SubmissionSummary>> {
        let directory = self.problem(id)?.join("submissions");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut summaries = vec![];
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.path().extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let submission_id = entry
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let path = self.submission_path(id, &submission_id)?;
            let cache = self
                .problem(id)?
                .join(format!("submission-summaries/{submission_id}.json"));
            let summary: SubmissionSummary = if cache.exists()
                && fs::metadata(&cache)?.modified()? >= fs::metadata(&path)?.modified()?
            {
                read_json(&cache)?
            } else {
                let summary = read_json::<SubmissionSummary>(&path)?;
                self.save_submission_summary(&summary)?;
                summary
            };
            summaries.push(summary);
        }
        summaries.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(summaries)
    }
    pub fn all_submission_summaries(&self) -> Result<Vec<(SubmissionSummary, String)>> {
        let directory = self.root.join("problems");
        if !directory.exists() {
            return Ok(vec![]);
        }
        let mut result = vec![];
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || !entry.path().join("submissions").is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            let name = self.manifest(&id)?.name;
            result.extend(
                self.submission_summaries(&id)?
                    .into_iter()
                    .map(|s| (s, name.clone())),
            );
        }
        result.sort_by(|(a, _), (b, _)| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
                .then_with(|| a.problem_id.cmp(&b.problem_id))
        });
        Ok(result)
    }
    pub fn records<T: DeserializeOwned>(&self, id: &str, kind: &str) -> Result<Vec<T>> {
        if !["reviews", "submissions", "validator-runs"].contains(&kind) {
            bail!("invalid record type");
        }
        let p = self.problem(id)?.join(kind);
        if !p.exists() {
            return Ok(vec![]);
        }
        fs::read_dir(p)?.map(|e| read_json(&e?.path())).collect()
    }
}
