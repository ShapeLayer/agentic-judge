use crate::model::*;
use anyhow::{Result, bail, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub struct Store {
    pub root: PathBuf,
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
        {
            return Ok(a.clone());
        }
        let a = Answer {
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
