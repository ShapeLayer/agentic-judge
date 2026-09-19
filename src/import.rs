//! Document-only imports. Importing never executes package code or certifies tests.
use crate::{
    engine::Engine,
    model::Manifest,
    storage::{Store, identifier},
};
use anyhow::{Context, Result, bail, ensure};
use scraper::{ElementRef, Html, Selector};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Default)]
struct Document {
    name: String,
    time: u64,
    memory: u64,
    tags: Vec<String>,
    sections: BTreeMap<String, String>,
    examples: Vec<(String, String)>,
    warnings: Vec<String>,
    assets: Vec<(String, Vec<u8>)>,
}

fn read(root: &Path, path: impl AsRef<Path>) -> Result<String> {
    let p = root.join(path).canonicalize()?;
    ensure!(p.starts_with(root), "source path outside package");
    ensure!(
        fs::metadata(&p)?.len() <= 16 * 1024 * 1024,
        "document too large"
    );
    Ok(fs::read_to_string(p)?)
}

impl Engine {
    pub fn import_document(&self, v: &Value) -> Result<Value> {
        let id = v["problem_id"].as_str().context("problem_id required")?;
        identifier(id)?;
        let destination = self.store.problem(id)?;
        ensure!(!destination.exists(), "problem already exists");
        let workspace = self.workspace_root()?;
        let path = workspace
            .join(v["path"].as_str().context("path required")?)
            .canonicalize()?;
        ensure!(path.starts_with(&workspace), "source outside workspace");
        let format = v["format"].as_str().unwrap_or("auto");
        let format = if format == "auto" {
            if path.is_dir() || path.extension().is_some_and(|x| x == "xml") {
                "polygon"
            } else {
                "boj_archive"
            }
        } else {
            format
        };
        let mut document = match format {
            "polygon" => polygon(&path, v["language"].as_str(), id)?,
            "boj_archive" => boj(
                &path,
                v["time_limit_ms"].as_u64(),
                v["memory_limit_mb"].as_u64(),
            )?,
            _ => bail!("format must be auto, polygon or boj_archive"),
        };
        for (key, slot) in [
            ("time_limit_ms", &mut document.time),
            ("memory_limit_mb", &mut document.memory),
        ] {
            if let Some(value) = v.get(key) {
                *slot = value
                    .as_u64()
                    .filter(|n| *n > 0)
                    .with_context(|| format!("{key} must be a positive integer"))?;
            }
        }
        ensure!(!document.name.trim().is_empty(), "empty problem title");
        ensure!(
            document.time > 0 && document.memory > 0,
            "limits must be positive"
        );
        ensure!(document.examples.len() <= 1000, "too many examples");
        // Build out of sight, then publish the complete directory in one rename.
        let staging = self
            .store
            .root
            .join(format!(".import-{}", crate::model::uuid()));
        let result = (|| -> Result<Manifest> {
            let store = Store::new(&staging)?;
            let mut m = store.create(id, &document.name)?;
            m.time_limit_ms = document.time;
            m.memory_limit_mb = document.memory;
            m.tags = document.tags;
            m.example_inputs = document.examples.len();
            m.example_outputs = m.example_inputs;
            let target = store.problem(id)?;
            for (section, text) in document.sections {
                fs::write(target.join(format!("statements/{section}.md")), text)?;
            }
            for (index, (input, output)) in document.examples.iter().enumerate() {
                fs::write(
                    target.join(format!("statements/example.{}.in", index + 1)),
                    input,
                )?;
                fs::write(
                    target.join(format!("statements/example.{}.out", index + 1)),
                    output,
                )?;
            }
            for (name, bytes) in document.assets {
                fs::create_dir_all(target.join("statements/assets"))?;
                fs::write(target.join("statements/assets").join(name), bytes)?;
            }
            store.save_manifest(&m)?;
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::rename(target, &destination)?;
            Ok(m)
        })();
        let _ = fs::remove_dir_all(staging);
        Ok(
            json!({"manifest":result?, "format":format, "warnings":document.warnings, "ready":false}),
        )
    }
}

struct BojMetadata {
    title: String,
    time: String,
    memory: String,
    tags: Vec<String>,
}
fn quantity(value: &str, units: &[(&str, f64)]) -> Result<u64> {
    let value = value.split('(').next().unwrap_or(value).trim();
    for (unit, multiplier) in units {
        if let Some(number) = value.trim().strip_suffix(unit) {
            let number: f64 = if *unit == "secondes" {
                number.trim().replace(',', ".").parse()?
            } else {
                number.trim().parse()?
            };
            let value = number * multiplier;
            ensure!(
                value.is_finite() && value > 0.0 && value < u64::MAX as f64,
                "invalid limit"
            );
            return Ok(value.ceil() as u64);
        }
    }
    bail!("unsupported limit: {value}")
}
fn boj(path: &Path, time: Option<u64>, memory: Option<u64>) -> Result<Document> {
    let source = read(path.parent().context("missing parent")?, path)?;
    let source = source.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let rest = source
        .strip_prefix("---\n")
        .context("YAML front matter required")?;
    let (metadata, body) = rest
        .split_once("\n---\n")
        .context("unclosed YAML front matter")?;
    // The archive emits literal scalar values (including unquoted colons in titles),
    // so normalize only the fields in our contract before parsing the tag list.
    let mut fields = BTreeMap::new();
    let mut tags_yaml = String::new();
    let mut in_tags = false;
    for line in metadata.lines() {
        if in_tags && (line.starts_with(' ') || line.starts_with('\t')) {
            tags_yaml.push_str(line);
            tags_yaml.push('\n');
            continue;
        }
        in_tags = false;
        if let Some((key, value)) = line.split_once(':') {
            if ["title", "time-limit", "memory-limit"].contains(&key) {
                ensure!(
                    fields.insert(key, value.trim().to_string()).is_none(),
                    "duplicate metadata {key}"
                );
            } else if key == "tags" {
                ensure!(tags_yaml.is_empty(), "duplicate tags");
                tags_yaml.push_str(value.trim());
                tags_yaml.push('\n');
                in_tags = true;
            }
        }
    }
    let metadata = BojMetadata {
        title: fields.remove("title").context("missing title")?,
        time: fields.remove("time-limit").context("missing time-limit")?,
        memory: fields
            .remove("memory-limit")
            .context("missing memory-limit")?,
        tags: if tags_yaml.trim().is_empty() {
            vec![]
        } else {
            serde_yaml::from_str(&tags_yaml)?
        },
    };
    let limit_notes = if time.is_some()
        || memory.is_some()
        || metadata.time.contains('(')
        || metadata.memory.contains('(')
    {
        Some(format!(
            "## 원본 제한\n\n시간: {}\n\n메모리: {}",
            metadata.time, metadata.memory
        ))
    } else {
        None
    };
    let mut d = Document {
        name: metadata.title,
        tags: metadata.tags,
        time: time
            .map(Ok)
            .unwrap_or_else(|| {
                quantity(
                    &metadata.time,
                    &[
                        ("초", 1000.0),
                        ("ms", 1.0),
                        ("seconds", 1000.0),
                        ("secondes", 1000.0),
                    ],
                )
            })
            .context("time_limit_ms override required for this source limit")?,
        memory: memory
            .map(Ok)
            .unwrap_or_else(|| {
                quantity(
                    &metadata.memory,
                    &[("MB", 1.0), ("GB", 1024.0), ("KB", 1.0 / 1024.0)],
                )
            })
            .context("memory_limit_mb override required for this source limit")?,
        ..Document::default()
    };
    // Parse headings through CommonMark so headings inside code blocks stay in code.
    use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
    let mut headings = vec![];
    let mut current = None;
    for (event, range) in Parser::new(body).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H2,
                ..
            }) => current = Some((range.start, range.end, String::new())),
            Event::Text(text) | Event::Code(text) if current.is_some() => {
                current.as_mut().unwrap().2.push_str(&text)
            }
            Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                if let Some(h) = current.take() {
                    headings.push(h);
                }
            }
            _ => {}
        }
    }
    let mut examples: BTreeMap<usize, (Option<String>, Option<String>)> = BTreeMap::new();
    for (index, (_, end, heading)) in headings.iter().enumerate() {
        let stop = headings.get(index + 1).map_or(body.len(), |h| h.0);
        let content = body[*end..stop].trim();
        let sample = heading
            .strip_prefix("예제 입력 ")
            .map(|n| (n, true))
            .or_else(|| heading.strip_prefix("예제 출력 ").map(|n| (n, false)));
        if let Some((number, input)) = sample {
            let number: usize = number.trim().parse().context("invalid example number")?;
            ensure!(number > 0 && number <= 1000, "invalid example number");
            let pair = examples.entry(number).or_default();
            let mut blocks = Vec::new();
            let mut current = None;
            for (event, range) in Parser::new(content).into_offset_iter() {
                match event {
                    Event::Start(Tag::CodeBlock(_)) => current = Some((String::new(), range)),
                    Event::Text(t) if current.is_some() => current.as_mut().unwrap().0.push_str(&t),
                    Event::End(TagEnd::CodeBlock) => {
                        if let Some(block) = current.take() {
                            blocks.push(block);
                        }
                    }
                    _ => {}
                }
            }
            if blocks.len() == 1 {
                let (code, range) = blocks.pop().unwrap();
                let extra = format!("{}{}", &content[..range.start], &content[range.end..]);
                if !extra.trim().is_empty() {
                    d.sections
                        .entry("notes".into())
                        .or_default()
                        .push_str(&format!("\n\n## {heading} 추가 본문\n\n{}", extra.trim()));
                }
                let slot = if input { &mut pair.0 } else { &mut pair.1 };
                ensure!(
                    slot.as_ref().is_none_or(|old| old == &code),
                    "conflicting duplicate example {number}"
                );
                *slot = Some(code);
            } else if !content.is_empty() {
                d.warnings.push(format!(
                    "Ambiguous {heading}: preserved in notes without guessing sample data"
                ));
                d.sections
                    .entry("notes".into())
                    .or_default()
                    .push_str(&format!("\n\n## 원본 {heading} (확인 필요)\n\n{content}"));
            }
            continue;
        }
        let section = match heading.as_str() {
            "문제" => "legend",
            "입력" => "input",
            "출력" => "output",
            _ => "notes",
        };
        let text = d.sections.entry(section.into()).or_default();
        if !content.is_empty() {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            if section == "notes" {
                text.push_str(&format!("## {heading}\n\n"));
            }
            text.push_str(content);
        }
    }
    if let Some(notes) = limit_notes {
        let text = d.sections.entry("notes".into()).or_default();
        text.push_str("\n\n");
        text.push_str(&notes);
    }
    for (number, (input, output)) in examples {
        match (input, output) {
            (Some(i), Some(o)) => {
                if number != d.examples.len() + 1 {
                    d.warnings.push(format!(
                        "Source example {number} registered as example {}",
                        d.examples.len() + 1
                    ));
                }
                d.examples.push((i, o));
            }
            (i, o) => {
                d.warnings.push(format!("Example {number} is incomplete in the source; preserved in notes, not registered as an example pair"));
                let notes = d.sections.entry("notes".into()).or_default();
                for (label, text) in [("입력", i), ("출력", o)] {
                    if let Some(text) = text {
                        notes.push_str(&format!(
                            "\n\n## 원본 예제 {label} {number} (불완전)\n\n{}",
                            fenced(&text)
                        ));
                    }
                }
            }
        }
    }
    if d.sections.get("legend").is_none_or(|s| s.is_empty()) {
        d.warnings
            .push("The source has an empty problem section".into());
    }
    Ok(d)
}

fn polygon(path: &Path, language: Option<&str>, id: &str) -> Result<Document> {
    let root = if path.is_dir() {
        path
    } else {
        path.parent().context("missing package directory")?
    };
    let xml = read(
        root,
        if path.is_dir() {
            root.join("problem.xml")
        } else {
            path.to_path_buf()
        },
    )?;
    let tree = roxmltree::Document::parse(&xml)?;
    let statements: Vec<_> = tree
        .descendants()
        .filter(|n| n.has_tag_name("statement") && n.attribute("type") == Some("text/html"))
        .collect();
    let chosen = if let Some(language) = language {
        statements
            .iter()
            .find(|n| n.attribute("language") == Some(language))
    } else {
        statements
            .iter()
            .find(|n| n.attribute("language") == Some("korean"))
            .or_else(|| {
                statements
                    .iter()
                    .find(|n| n.attribute("language") == Some("english"))
            })
            .or_else(|| statements.first())
    }
    .context("Polygon HTML statement unavailable for requested language; export HTML statements")?;
    let language = chosen
        .attribute("language")
        .context("statement language missing")?;
    let html_path = root.join(chosen.attribute("path").context("statement path missing")?);
    let html = Html::parse_document(&read(root, &html_path)?);
    let name = tree
        .descendants()
        .find(|n| n.has_tag_name("name") && n.attribute("language") == Some(language))
        .and_then(|n| n.attribute("value"))
        .context("problem name missing")?;
    let judging = tree
        .descendants()
        .find(|n| n.has_tag_name("judging"))
        .context("judging missing")?;
    let testset = judging
        .children()
        .find(|n| n.has_tag_name("testset"))
        .context("testset missing")?;
    let field = |name| -> Result<u64> {
        Ok(testset
            .children()
            .find(|n| n.has_tag_name(name))
            .and_then(|n| n.text())
            .with_context(|| format!("missing {name}"))?
            .trim()
            .parse()?)
    };
    let mut d = Document {
        name: name.into(),
        time: field("time-limit")?,
        memory: field("memory-limit")?.div_ceil(1024 * 1024),
        ..Document::default()
    };
    d.tags = tree
        .descendants()
        .filter(|n| n.has_tag_name("tag") && n.parent().is_some_and(|p| p.has_tag_name("tags")))
        .filter_map(|n| n.attribute("value"))
        .map(String::from)
        .collect();
    for (section, class) in [
        ("legend", "legend"),
        ("input", "input-specification"),
        ("output", "output-specification"),
        ("notes", "note"),
    ] {
        let selector = Selector::parse(&format!(".{class}")).unwrap();
        let mut text = String::new();
        for node in html.select(&selector) {
            text.push_str(&html_markdown(
                node,
                root,
                html_path.parent().unwrap(),
                id,
                &mut d,
            )?);
        }
        d.sections.insert(section.into(), text.trim().into());
    }
    ensure!(
        d.sections.get("legend").is_some_and(|s| !s.is_empty()),
        "missing Polygon legend"
    );
    // Statement-specific sample files preserve whitespace exactly, unlike HTML PRE.
    let props_path = format!("statements/{language}/problem-properties.json");
    if root.join(&props_path).exists() {
        let props: Value = serde_json::from_str(&read(root, props_path)?)?;
        for sample in props["sampleTests"]
            .as_array()
            .context("sampleTests missing")?
        {
            let sample_value = |key: &str, file_key: &str| -> Result<String> {
                if let Some(file) = sample[file_key].as_str() {
                    read(root, format!("statements/{language}/{file}"))
                } else {
                    Ok(sample[key].as_str().context("sample text missing")?.into())
                }
            };
            d.examples.push((
                sample_value("input", "inputFile")?,
                sample_value("output", "outputFile")?,
            ));
        }
    } else {
        let inputs: Vec<_> = html
            .select(&Selector::parse(".sample-test .input pre").unwrap())
            .map(pre_text)
            .collect();
        let outputs: Vec<_> = html
            .select(&Selector::parse(".sample-test .output pre").unwrap())
            .map(pre_text)
            .collect();
        ensure!(
            inputs.len() == outputs.len(),
            "incomplete Polygon HTML examples"
        );
        d.examples = inputs.into_iter().zip(outputs).collect();
        d.warnings
            .push("Polygon sample files unavailable; examples extracted from HTML".into());
    }
    Ok(d)
}

fn pre_text(node: ElementRef<'_>) -> String {
    let mut out = String::new();
    for child in node.children() {
        if let Some(element) = ElementRef::wrap(child) {
            if element.value().name() == "br" {
                out.push('\n');
            } else {
                out.push_str(&pre_text(element));
            }
        } else if let Some(text) = child.value().as_text() {
            out.push_str(text);
        }
    }
    out
}
fn polygon_text(text: &str) -> String {
    let mut out = String::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("$$$") {
        out.push_str(&escape_markdown(&remaining[..start]));
        remaining = &remaining[start..];
        let delimiter = if remaining.starts_with("$$$$$$") {
            "$$$$$$"
        } else {
            "$$$"
        };
        let tail = &remaining[delimiter.len()..];
        if let Some(end) = tail.find(delimiter) {
            let marker = if delimiter.len() == 6 { "$$" } else { "$" };
            out.push_str(marker);
            out.push_str(&tail[..end]);
            out.push_str(marker);
            remaining = &tail[end + delimiter.len()..];
        } else {
            out.push_str(&escape_markdown(remaining));
            return out;
        }
    }
    out.push_str(&escape_markdown(remaining));
    out
}
fn escape_markdown(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if "\\`*_{}[]<>()#+!$|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn fenced(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest + 1));
    format!(
        "{fence}\n{text}{}{fence}\n",
        if text.ends_with('\n') { "" } else { "\n" }
    )
}

fn html_markdown(
    node: ElementRef<'_>,
    root: &Path,
    base: &Path,
    id: &str,
    d: &mut Document,
) -> Result<String> {
    let tag = node.value().name();
    if ["script", "style"].contains(&tag) || node.value().classes().any(|c| c == "section-title") {
        return Ok(String::new());
    }
    if tag == "pre" {
        return Ok(format!("\n\n{}\n", fenced(&pre_text(node))));
    }
    if tag == "img" {
        let src = node.value().attr("src").context("image source missing")?;
        let url = if src.starts_with("https://") || src.starts_with("http://") {
            src.to_string()
        } else {
            let p = base.join(src).canonicalize()?;
            ensure!(p.starts_with(root), "image outside package");
            ensure!(
                fs::metadata(&p)?.len() <= 16 * 1024 * 1024,
                "image too large"
            );
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            ensure!(
                ["png", "jpg", "jpeg", "gif", "webp"].contains(&ext.as_str()),
                "unsupported image format {ext}"
            );
            let bytes = fs::read(p)?;
            let name = format!("{}.{}", crate::model::hash(&bytes), ext);
            let url = format!("/api/problems/{id}/assets/{name}");
            d.assets.push((name, bytes));
            url
        };
        return Ok(format!("![](<{url}>)"));
    }
    let mut text = String::new();
    for child in node.children() {
        if let Some(element) = ElementRef::wrap(child) {
            text.push_str(&html_markdown(element, root, base, id, d)?);
        } else if let Some(t) = child.value().as_text() {
            text.push_str(&polygon_text(t));
        }
    }
    Ok(match tag {
        "p" | "div" | "center" => format!("\n\n{text}\n\n"),
        "br" => "  \n".into(),
        "strong" | "b" => format!("**{text}**"),
        "em" | "i" => format!("*{text}*"),
        "code" | "tt" => format!("`{text}`"),
        "li" => format!("\n- {}\n", text.trim()),
        "td" | "th" => format!(" {} |", text.trim()),
        "tr" => format!("|{text}\n"),
        "table" => {
            let text = text.trim();
            let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
            let columns = node
                .select(&Selector::parse("tr").unwrap())
                .next()
                .map_or(0, |r| {
                    r.children()
                        .filter_map(ElementRef::wrap)
                        .filter(|e| ["td", "th"].contains(&e.value().name()))
                        .count()
                });
            format!("\n\n{first}\n|{}\n{rest}\n\n", " --- |".repeat(columns))
        }
        "a" => {
            if let Some(href) = node.value().attr("href") {
                format!("[{text}](<{href}>)")
            } else {
                text
            }
        }
        "sub" | "sup" => format!("<{tag}>{text}</{tag}>"),
        _ => text,
    })
}

#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    #[ignore = "requires optional local source archives"]
    fn parse_local_reference_documents() {
        let archive =
            std::env::var("BOJ_ARCHIVE").expect("set BOJ_ARCHIVE to archive problems directory");
        let mut paths: Vec<_> = fs::read_dir(archive)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        paths.sort();
        let mut errors = vec![];
        let mut overrides = 0;
        let mut warnings = 0;
        for path in &paths {
            match boj(path, None, None) {
                Ok(document) => warnings += document.warnings.len(),
                Err(e) if e.to_string().contains("override required") => {
                    overrides += 1;
                    if let Err(e) = boj(path, Some(1000), Some(256)) {
                        errors.push(format!("{}: {e:#}", path.display()));
                    }
                }
                Err(e) => errors.push(format!("{}: {e:#}", path.display())),
            }
        }
        eprintln!("Explicit limit overrides required: {overrides}; source warnings: {warnings}");
        eprintln!("BOJ documents: {}, failures: {}", paths.len(), errors.len());
        for error in errors.iter().take(40) {
            eprintln!("{error}");
        }
        let refs = Path::new(env!("CARGO_MANIFEST_DIR")).join("_PREWORK_REFS");
        let mut count = 0;
        for event in fs::read_dir(refs).unwrap() {
            let packages = event.unwrap().path().join("polygon-packages");
            if !packages.is_dir() {
                continue;
            }
            for package in fs::read_dir(packages).unwrap() {
                let path = package.unwrap().path();
                if !path.is_dir() {
                    continue;
                }
                if let Err(e) = polygon(&path, None, "reference") {
                    errors.push(format!("{}: {e:#}", path.display()));
                    eprintln!("{}: {e:#}", path.display());
                }
                count += 1;
            }
        }
        eprintln!("Polygon documents: {count}");
        assert!(
            errors.is_empty(),
            "{} source documents failed",
            errors.len()
        );
    }
}
