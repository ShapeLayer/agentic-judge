//! Versioned, data-only ZIP transfer. Imported code is never executed.
use crate::{engine::Engine, model::*, storage::*};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
};

pub const LIMIT: usize = 32 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
struct Case {
    suite: String,
    number: u64,
    input: String,
    output: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Package {
    version: u32,
    kind: String,
    manifest: Option<Manifest>,
    files: BTreeMap<String, Vec<u8>>,
    answers: Vec<Answer>,
    suites: Vec<String>,
    tests: Vec<Case>,
}
impl Engine {
    pub fn export_archive(
        &self,
        kind: &str,
        id: &str,
        suite: &str,
        number: u64,
    ) -> Result<Vec<u8>> {
        let lock = self.archive_lock()?;
        let m = self.store.manifest(id)?;
        ensure!(
            ["problem", "suite", "test"].contains(&kind),
            "invalid archive kind"
        );
        if kind != "problem" {
            identifier(suite)?;
            ensure!(m.suites.contains(&suite.to_owned()), "test suite not found");
        }
        let mut p = Package {
            version: 1,
            kind: kind.into(),
            manifest: None,
            files: BTreeMap::new(),
            answers: vec![],
            suites: if kind == "problem" {
                m.suites.clone()
            } else {
                vec![suite.into()]
            },
            tests: vec![],
        };
        for t in self.store.tests(id)? {
            if kind != "problem" && (t.suite != suite || kind == "test" && t.number != number) {
                continue;
            }
            let out = self.store.test_path(id, &t.suite, t.number, "out")?;
            p.tests.push(Case {
                input: fs::read_to_string(self.store.test_path(id, &t.suite, t.number, "in")?)?,
                output: if out.exists() {
                    Some(fs::read_to_string(out)?)
                } else {
                    None
                },
                suite: t.suite,
                number: t.number,
            });
        }
        ensure!(kind != "test" || p.tests.len() == 1, "test not found");
        if kind == "problem" {
            let root = self.store.problem(id)?;
            fn collect(
                root: &std::path::Path,
                dir: &std::path::Path,
                files: &mut BTreeMap<String, Vec<u8>>,
            ) -> Result<()> {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let ty = entry.file_type()?;
                    ensure!(!ty.is_symlink(), "symlinks cannot be exported");
                    if ty.is_dir() {
                        collect(root, &entry.path(), files)?;
                    } else {
                        files.insert(
                            entry.path().strip_prefix(root)?.to_string_lossy().into(),
                            fs::read(entry.path())?,
                        );
                    }
                }
                Ok(())
            }
            collect(&root, &root.join("statements"), &mut p.files)?;
            if root.join("evaluate").exists() {
                collect(&root, &root.join("evaluate"), &mut p.files)?;
            }
            if root.join("answers").exists() {
                for e in fs::read_dir(root.join("answers"))? {
                    p.answers
                        .push(self.store.answer(id, &e?.file_name().to_string_lossy())?);
                }
            }
            p.manifest = Some(m);
        }
        let bytes = serde_json::to_vec(&p)?;
        ensure!(bytes.len() <= LIMIT, "archive exceeds 32 MiB");
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(
            "package.json",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )?;
        zip.write_all(&bytes)?;
        drop(lock);
        Ok(zip.finish()?.into_inner())
    }
    fn archive_lock(&self) -> Result<fs::File> {
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.store.root.join(".lock"))?;
        lock.lock_exclusive()?;
        Ok(lock)
    }
    pub fn import_archive(
        &self,
        bytes: &[u8],
        kind: &str,
        id: &str,
        suite: &str,
        number: u64,
    ) -> Result<Value> {
        let _lock = self.archive_lock()?;
        identifier(id)?;
        ensure!(bytes.len() <= LIMIT, "archive exceeds 32 MiB");
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
        ensure!(zip.len() == 1, "ZIP must contain only package.json");
        let file = zip.by_name("package.json")?;
        ensure!(
            file.size() <= LIMIT as u64,
            "expanded archive exceeds 32 MiB"
        );
        let mut data = Vec::new();
        file.take(LIMIT as u64 + 1).read_to_end(&mut data)?;
        ensure!(data.len() <= LIMIT, "expanded archive exceeds 32 MiB");
        let mut p: Package = serde_json::from_slice(&data)?;
        ensure!(
            p.version == 1 && p.kind == kind && ["problem", "suite", "test"].contains(&kind),
            "archive type or version mismatch"
        );
        if kind != "problem" {
            identifier(suite)?;
            ensure!(
                p.manifest.is_none()
                    && p.files.is_empty()
                    && p.answers.is_empty()
                    && p.suites.len() == 1,
                "invalid test archive"
            );
        }
        if kind == "test" {
            ensure!(p.tests.len() == 1 && number > 0, "invalid individual test");
            p.tests[0].number = number;
        }
        if kind != "problem" {
            p.suites = vec![suite.into()];
            for t in &mut p.tests {
                t.suite = suite.into();
            }
        }
        let mut seen = std::collections::HashSet::new();
        for s in &p.suites {
            identifier(s)?;
            ensure!(seen.insert(s.clone()), "duplicate suite");
        }
        let mut seen = std::collections::HashSet::new();
        for t in &p.tests {
            identifier(&t.suite)?;
            ensure!(
                t.number > 0
                    && p.suites.contains(&t.suite)
                    && seen.insert((t.suite.clone(), t.number)),
                "invalid or duplicate test"
            );
        }
        let destination = self.store.problem(id)?;
        let mut manifest = if kind == "problem" {
            ensure!(
                !destination.exists(),
                "problem already exists; choose a different ID"
            );
            let mut m = p.manifest.take().context("problem manifest missing")?;
            identifier(&m.id)?;
            ensure!(
                m.suites == p.suites
                    && m.time_limit_ms > 0
                    && m.time_limit_ms <= 300000
                    && m.memory_limit_mb > 0
                    && m.example_inputs <= 1000
                    && m.example_inputs == m.example_outputs,
                "invalid manifest"
            );
            for (path, bytes) in &mut p.files {
                ensure!(
                    (path.starts_with("statements/") || path.starts_with("evaluate/"))
                        && !path.contains('\\')
                        && path
                            .split('/')
                            .all(|s| !s.is_empty() && s != "." && s != ".."),
                    "invalid statement path"
                );
                if path.ends_with(".md") {
                    *bytes = String::from_utf8(bytes.clone())?
                        .replace(
                            &format!("/api/problems/{}/assets/", m.id),
                            &format!("/api/problems/{id}/assets/"),
                        )
                        .into_bytes();
                }
            }
            for section in ["legend", "input", "output", "notes"] {
                ensure!(
                    p.files.contains_key(&format!("statements/{section}.md")),
                    "statement missing"
                );
            }
            for n in 1..=m.example_inputs {
                for ext in ["in", "out"] {
                    ensure!(
                        p.files
                            .contains_key(&format!("statements/example.{n}.{ext}")),
                        "example missing"
                    );
                }
            }
            let mut ids = std::collections::HashSet::new();
            for a in &p.answers {
                uuid::Uuid::parse_str(&a.id)?;
                ensure!(ids.insert(a.id.clone()), "duplicate answer");
            }
            for aid in [&m.main_solution, &m.validator].into_iter().flatten() {
                ensure!(ids.contains(aid), "referenced answer missing");
            }
            m.id = id.into();
            m
        } else {
            let m = self.store.manifest(id)?;
            if kind == "suite" {
                ensure!(
                    !m.suites.contains(&suite.to_owned())
                        && !destination.join("tests").join(suite).exists(),
                    "test suite already exists; choose a different name"
                );
            } else {
                for ext in ["in", "out", "json"] {
                    ensure!(
                        !self.store.test_path(id, suite, number, ext)?.exists(),
                        "test already exists; choose a different number"
                    );
                }
            }
            m
        };
        let staging = self.store.root.join(format!(".archive-{}", uuid()));
        let result = (|| -> Result<Value> {
            let stage = Store::new(&staging)?;
            stage.create(id, &manifest.name)?;
            if kind != "problem" {
                manifest.suites = p.suites.clone();
            }
            stage.save_manifest(&manifest)?;
            let root = stage.problem(id)?;
            for s in &p.suites {
                fs::create_dir_all(root.join("tests").join(s))?;
            }
            for (path, bytes) in &p.files {
                atomic(&root.join(path), bytes)?;
            }
            for a in &p.answers {
                write_json(&root.join(format!("answers/{}/current.json", a.id)), a)?;
                write_json(
                    &root.join(format!("answers/{}/revisions/{}.json", a.id, a.revision)),
                    a,
                )?;
            }
            for t in &p.tests {
                let mut meta = stage.put_test(id, &t.suite, t.number, &t.input)?;
                if let Some(output) = &t.output {
                    atomic(
                        &stage.test_path(id, &t.suite, t.number, "out")?,
                        output.as_bytes(),
                    )?;
                    meta.output_hash = Some(hash(output));
                    write_json(&stage.test_path(id, &t.suite, t.number, "json")?, &meta)?;
                }
            }
            if kind == "problem" {
                fs::create_dir_all(destination.parent().unwrap())?;
                fs::rename(root, &destination)?;
            } else {
                let mut current = self.store.manifest(id)?;
                let target = destination.join("tests").join(suite);
                if kind == "suite" || !target.exists() {
                    fs::rename(root.join("tests").join(suite), &target)?;
                } else {
                    for ext in ["in", "out", "json"] {
                        let source = stage.test_path(id, suite, number, ext)?;
                        if source.exists() {
                            fs::rename(source, self.store.test_path(id, suite, number, ext)?)?;
                        }
                    }
                }
                if !current.suites.contains(&suite.to_owned()) {
                    current.suites.push(suite.into());
                    self.store.save_manifest(&current)?;
                }
            }
            Ok(json!({"problem_id": id, "kind":kind, "tests":p.tests.len(), "ready":false}))
        })();
        let _ = fs::remove_dir_all(staging);
        result
    }
}
