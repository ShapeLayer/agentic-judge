//! Bulk document-only import using the same parser as the UI and MCP.
use agentic_judge::{config, engine::Engine};
use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{env, fs, path::PathBuf};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    ensure!(
        args.len() == 2,
        "usage: import_boj_archive <config> <archive>"
    );
    let archive = PathBuf::from(&args[1]).canonicalize()?;
    let config = config::load(Some(&PathBuf::from(&args[0])), Some(&archive), false)?;
    let engine = Engine::new(config)?;
    let mut files = fs::read_dir(archive.join("problems"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "md"));
    files.sort();
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = vec![];
    let mut warnings = vec![];
    for (index, path) in files.iter().enumerate() {
        let filename = path
            .file_name()
            .context("missing filename")?
            .to_string_lossy();
        let number = filename.split('번').next().unwrap_or("");
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            errors.push(json!({"file":filename,"error":"unrecognized problem number"}));
            continue;
        }
        let id = format!("boj-{number}");
        if engine.store.problem(&id)?.exists() {
            skipped += 1;
            continue;
        }
        match engine.import_document(&json!({
            "problem_id":id,"format":"boj_archive",
            "path":path.strip_prefix(&archive)?.to_string_lossy()
        })) {
            Ok(result) => {
                imported += 1;
                if result["warnings"].as_array().is_some_and(|w| !w.is_empty()) {
                    warnings.push(
                        json!({"problem_id":id,"file":filename,"warnings":result["warnings"]}),
                    );
                }
            }
            Err(error) => {
                // Preserve documents with nonnumeric limits without inventing judge settings.
                let raw = engine.store.root.join("boj-unimported");
                fs::create_dir_all(&raw)?;
                fs::copy(
                    path,
                    raw.join(path.file_name().context("missing filename")?),
                )?;
                errors.push(json!({"file":filename,"error":format!("{error:#}")}));
            }
        }
        if (index + 1) % 1000 == 0 {
            eprintln!(
                "{}/{} processed; {} imported; {} errors",
                index + 1,
                files.len(),
                imported,
                errors.len()
            );
        }
    }
    let report = json!({"source":archive,"total":files.len(),"imported":imported,
        "skipped_existing":skipped,"errors":errors,"warnings":warnings});
    let report_path = engine.store.root.join("boj-import-report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "Imported {imported}; existing {skipped}; errors {}; source warnings {}. Report: {}",
        errors.len(),
        warnings.len(),
        report_path.display()
    );
    Ok(())
}
