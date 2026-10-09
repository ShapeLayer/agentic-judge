use crate::{model::*, runtime::Runtime, storage::*};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::fs;

#[derive(Clone)]
pub struct Engine {
    pub store: Store,
    pub runtime: Runtime,
}
impl Engine {
    pub fn new(config: Config) -> Result<Self> {
        let store = Store::new(&config.storage_root)?;
        fs::create_dir_all(&config.runtime_root)?;
        let runtime_root = config.runtime_root.canonicalize()?;
        ensure!(
            !runtime_root.starts_with(&store.root) && !store.root.starts_with(&runtime_root),
            "runtime and storage roots must be separate directories"
        );
        Ok(Self {
            store,
            runtime: Runtime { config },
        })
    }
    pub fn regenerate(&self, id: &str) -> Result<Value> {
        let m = self.store.manifest(id)?;
        let generation = self.store.generation(id)?;
        let a = self.store.answer(
            id,
            m.main_solution
                .as_deref()
                .context("main solution missing")?,
        )?;
        let tests = self.store.tests(id)?;
        let mut count = 0;
        // Invalidate first; a failed generation can never leave the problem marked ready.
        for mut t in tests.clone() {
            t.generation = None;
            t.generation_error = None;
            t.validation_status = None;
            write_json(&self.store.test_path(id, &t.suite, t.number, "json")?, &t)?;
        }
        let p = self.runtime.prepare(&a.language, &a.source)?;
        let validator = m
            .validator
            .as_ref()
            .map(|v| self.store.answer(id, v))
            .transpose()?;
        let vp = validator
            .as_ref()
            .map(|v| self.runtime.prepare(&v.language, &v.source))
            .transpose()?;
        for mut t in tests {
            let input = fs::read_to_string(self.store.test_path(id, &t.suite, t.number, "in")?)?;
            t.input_hash = hash(&input);
            if let Some(vp) = &vp {
                let result =
                    self.runtime
                        .run(vp, &input, &[], m.time_limit_ms, m.memory_limit_mb)?;
                t.validation_status = Some(
                    if result.verdict == "AC" {
                        "valid"
                    } else {
                        "invalid"
                    }
                    .into(),
                );
                if result.verdict != "AC" {
                    t.generation_error = Some(format!(
                        "validator rejected: {} {}",
                        result.verdict, result.stderr
                    ));
                    write_json(&self.store.test_path(id, &t.suite, t.number, "json")?, &t)?;
                    anyhow::bail!(
                        "validator rejected {}/{}: {}",
                        t.suite,
                        t.number,
                        result.stderr
                    );
                }
            }
            let result = self
                .runtime
                .run(&p, &input, &[], m.time_limit_ms, m.memory_limit_mb)?;
            if result.verdict != "AC" {
                t.generation_error = Some(format!(
                    "main solution failed: {} {}",
                    result.verdict, result.stderr
                ));
                write_json(&self.store.test_path(id, &t.suite, t.number, "json")?, &t)?;
                anyhow::bail!(
                    "main solution failed {}/{}: {} {}",
                    t.suite,
                    t.number,
                    result.verdict,
                    result.stderr
                );
            }
            atomic(
                &self.store.test_path(id, &t.suite, t.number, "out")?,
                result.stdout.as_bytes(),
            )?;
            t.output_hash = Some(hash(&result.stdout));
            t.generation = Some(generation.clone());
            t.validator_hash = validator
                .as_ref()
                .map(|v| hash(serde_json::to_vec(v).unwrap()));
            write_json(&self.store.test_path(id, &t.suite, t.number, "json")?, &t)?;
            count += 1;
        }
        Ok(json!({"generated":count,"ready":self.store.ready(id)?,"generation":generation}))
    }
    fn create_submission(
        &self,
        id: &str,
        language: &str,
        source: &str,
        preparation: Option<PreparationRun>,
    ) -> Result<Submission> {
        ensure!(
            self.store.ready(id)?,
            "problem is not ready; regenerate answers"
        );
        self.store.manifest(id)?;
        ensure!(
            self.runtime.config.languages.contains_key(language),
            "unsupported language"
        );
        let submission = Submission {
            purpose: if preparation.is_some() {
                SubmissionPurpose::Preparation
            } else {
                SubmissionPurpose::Solve
            },
            preparation,
            format_version: 2,
            id: uuid(),
            problem_id: id.into(),
            language: language.into(),
            source: source.into(),
            created_at: now(),
            generation: self.store.generation(id)?,
            verdict: "JUDGING".into(),
            cases: vec![],
        };
        self.save_submission(&submission)?;
        Ok(submission)
    }
    pub fn submit(&self, id: &str, language: &str, source: &str) -> Result<Submission> {
        self.judge_submission(self.create_submission(id, language, source, None)?)
    }
    pub fn submit_preparation(
        &self,
        id: &str,
        language: &str,
        source: &str,
        context: PreparationRun,
    ) -> Result<Submission> {
        self.judge_submission(self.create_submission(id, language, source, Some(context))?)
    }
    pub fn submit_async(&self, id: &str, language: &str, source: &str) -> Result<Submission> {
        self.submit_async_with_context(id, language, source, None)
    }
    pub fn submit_async_with_context(
        &self,
        id: &str,
        language: &str,
        source: &str,
        preparation: Option<PreparationRun>,
    ) -> Result<Submission> {
        let submission = self.create_submission(id, language, source, preparation)?;
        let engine = self.clone();
        let pending = submission.clone();
        std::thread::Builder::new()
            .name(format!("judge-{}", submission.id))
            .spawn(move || {
                if let Err(error) = engine.judge_submission(pending.clone()) {
                    eprintln!("judging {} failed: {error:#}", pending.id);
                    let mut failed = pending;
                    failed.verdict = "JUDGE_ERROR".into();
                    if let Err(error) = engine.save_submission(&failed) {
                        eprintln!("saving failed submission {}: {error:#}", failed.id);
                    }
                }
            })
            .inspect_err(|_| {
                let mut failed = submission.clone();
                failed.verdict = "JUDGE_ERROR".into();
                let _ = self.save_submission(&failed);
            })?;
        Ok(submission)
    }
    fn judge_submission(&self, mut submission: Submission) -> Result<Submission> {
        let id = submission.problem_id.as_str();
        let m = self.store.manifest(id)?;
        ensure!(
            self.store.ready(id)? && self.store.generation(id)? == submission.generation,
            "problem changed before judging"
        );
        submission.verdict = "AC".into();
        match self
            .runtime
            .prepare(&submission.language, &submission.source)
        {
            Err(e) => {
                submission.verdict = "CE".into();
                submission.cases.push(CaseResult {
                    suite: String::new(),
                    number: 0,
                    result: RunResult {
                        verdict: "CE".into(),
                        time_ms: 0,
                        memory_kb: 0,
                        stdout: String::new(),
                        stderr: e.to_string(),
                        exit_code: None,
                    },
                    checker_message: String::new(),
                    input: String::new(),
                    expected_output: String::new(),
                    input_hash: None,
                    expected_output_hash: None,
                });
            }
            Ok(p) => {
                let checker = if m.checker.starts_with("./evaluate/") {
                    let path = self.store.problem(id)?.join(&m.checker);
                    let canonical = path.canonicalize()?;
                    ensure!(
                        canonical.starts_with(self.store.problem(id)?.canonicalize()?),
                        "checker outside problem"
                    );
                    Some(self.runtime.prepare_checker(
                        &self.store.problem(id)?,
                        &fs::read_to_string(canonical)?,
                    )?)
                } else if let Some(name) = m.checker.strip_prefix("testlib:") {
                    identifier(name)?;
                    let source = crate::builtin::COMMON_CHECKERS
                        .iter()
                        .find(|(id, _)| *id == name)
                        .context("unknown built-in testlib checker")?
                        .1;
                    Some(
                        self.runtime
                            .prepare_checker(&self.store.problem(id)?, source)?,
                    )
                } else {
                    ensure!(
                        ["tokens", "exact", "lines"].contains(&m.checker.as_str()),
                        "unknown checker"
                    );
                    None
                };
                for t in self.store.tests(id)? {
                    let input =
                        fs::read_to_string(self.store.test_path(id, &t.suite, t.number, "in")?)?;
                    let expected =
                        fs::read_to_string(self.store.test_path(id, &t.suite, t.number, "out")?)?;
                    let mut result =
                        self.runtime
                            .run(&p, &input, &[], m.time_limit_ms, m.memory_limit_mb)?;
                    let mut message = String::new();
                    if result.verdict == "AC" {
                        if let Some(checker) = &checker {
                            fs::write(checker.dir.join("input.txt"), &input)?;
                            fs::write(checker.dir.join("output.txt"), &result.stdout)?;
                            fs::write(checker.dir.join("answer.txt"), &expected)?;
                            let check = self.runtime.run(
                                checker,
                                "",
                                &["input.txt".into(), "output.txt".into(), "answer.txt".into()],
                                m.time_limit_ms,
                                m.memory_limit_mb,
                            )?;
                            message = check.stderr;
                            result.verdict = match check.exit_code {
                                Some(0) if check.verdict == "AC" => "AC",
                                Some(1) => "WA",
                                Some(2) => "PE",
                                _ => "CHECKER_ERROR",
                            }
                            .into();
                        } else {
                            let matched = match m.checker.as_str() {
                                "exact" => result.stdout == expected,
                                "lines" => result
                                    .stdout
                                    .lines()
                                    .map(str::trim_end)
                                    .eq(expected.lines().map(str::trim_end)),
                                _ => result
                                    .stdout
                                    .split_whitespace()
                                    .eq(expected.split_whitespace()),
                            };
                            if !matched {
                                result.verdict = "WA".into();
                            }
                        }
                    }
                    if submission.verdict == "AC" && result.verdict != "AC" {
                        submission.verdict = result.verdict.clone();
                    }
                    let mut case = CaseResult {
                        suite: t.suite,
                        number: t.number,
                        result,
                        checker_message: message,
                        input,
                        expected_output: expected,
                        input_hash: Some(t.input_hash),
                        expected_output_hash: t.output_hash,
                    };
                    self.store.persist_case_data(
                        id,
                        &submission.id,
                        submission.cases.len(),
                        &mut case,
                    )?;
                    submission.cases.push(case);
                }
            }
        }
        if submission.verdict == "CE" {
            self.store
                .persist_case_data(id, &submission.id, 0, &mut submission.cases[0])?;
        }
        self.save_submission(&submission)?;
        Ok(submission)
    }
    fn save_submission(&self, submission: &Submission) -> Result<()> {
        write_json(
            &self
                .store
                .problem(&submission.problem_id)?
                .join(format!("submissions/{}.json", submission.id)),
            submission,
        )?;
        self.store
            .save_submission_summary(&SubmissionSummary::from(submission))
    }
    pub fn fingerprint(&self, id: &str, target: &str) -> Result<String> {
        let m = self.store.manifest(id)?;
        let statements = ["legend", "input", "output", "notes"].map(|s| {
            fs::read_to_string(
                self.store
                    .problem(id)
                    .unwrap()
                    .join(format!("statements/{s}.md")),
            )
            .unwrap_or_default()
        });
        let object = if let Some(a) = target.strip_prefix("answer:") {
            serde_json::to_value(self.store.answer(id, a)?)?
        } else {
            let parts: Vec<_> = target.split(':').collect();
            ensure!(parts.len() == 3 && parts[0] == "test", "invalid target");
            let n = parts[2].parse()?;
            json!({"input":fs::read_to_string(self.store.test_path(id,parts[1],n,"in")?)?,"output":fs::read_to_string(self.store.test_path(id,parts[1],n,"out")?)?,"generation":self.store.generation(id)?})
        };
        Ok(hash(serde_json::to_vec(&(m, statements, object))?))
    }
    pub fn trust(&self, id: &str, target: &str) -> Result<Value> {
        let fp = self.fingerprint(id, target)?;
        let reviews: Vec<Review> = self.store.records(id, "reviews")?;
        let current: Vec<_> = reviews
            .iter()
            .filter(|r| r.target == target && r.fingerprint == fp)
            .collect();
        Ok(
            json!({"target":target,"fingerprint":fp,"valid":current.iter().filter(|r|r.status=="valid").count(),"invalid":current.iter().filter(|r|r.status=="invalid").count(),"uncertain":current.iter().filter(|r|r.status=="uncertain").count(),"errors":current.iter().filter(|r|r.status=="error").count(),"reviews":current,"historical_count":reviews.iter().filter(|r|r.target==target && r.fingerprint!=fp).count()}),
        )
    }
}
