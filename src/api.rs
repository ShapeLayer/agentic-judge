use crate::{engine::Engine, model::*, storage::*};
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{fs, io::Read};
fn s<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str().with_context(|| format!("missing string {k}"))
}
fn n(v: &Value, k: &str) -> Result<u64> {
    v[k].as_u64()
        .with_context(|| format!("missing positive integer {k}"))
}

impl Engine {
    pub fn call(&self, name: &str, v: Value) -> Result<Value> {
        // Shared advisory lock serializes HTTP/MCP writers across processes.
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.store.root.join(".lock"))?;
        lock.lock_exclusive()?;
        self.dispatch(name, v)
    }
    fn dispatch(&self, name: &str, v: Value) -> Result<Value> {
        let id = v["problem_id"].as_str().unwrap_or("");
        match name {
            "draft_get" | "draft_save" => {
                self.store.manifest(id)?;
                let language = s(&v, "language")?;
                ensure!(
                    self.runtime.config.languages.contains_key(language),
                    "unsupported language"
                );
                let scope = self
                    .runtime
                    .config
                    .workspace_root
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned());
                let key = hash(serde_json::to_vec(&(scope, id, language))?);
                let directory = self.store.root.join("drafts");
                let path = directory.join(format!("{key}.json"));
                if name == "draft_save" {
                    let source = s(&v, "source")?;
                    ensure!(source.len() <= 3 * 1024 * 1024, "draft too large");
                    fs::create_dir_all(directory)?;
                    let draft = json!({"problem_id":id,"language":language,"source":source,"updated_at":now()});
                    write_json(&path, &draft)?;
                    Ok(draft)
                } else if path.exists() {
                    read_json::<Value>(&path)
                } else {
                    Ok(Value::Null)
                }
            }
            "problems_list" => Ok(json!(self.store.list()?)),
            "problem_create" => Ok(json!(
                self.store.create(s(&v, "problem_id")?, s(&v, "name")?)?
            )),
            "problem_import" => self.import_document(&v),
            "problem_get" => {
                let mut statements = json!({});
                for section in ["legend", "input", "output", "notes"] {
                    let md = fs::read_to_string(
                        self.store
                            .problem(id)?
                            .join(format!("statements/{section}.md")),
                    )?;
                    statements[section] = json!({"markdown":md,"html":render(&md)});
                }
                Ok({
                    let m = self.store.manifest(id)?;
                    let mut examples = vec![];
                    for number in 1..=m.example_inputs {
                        examples.push(json!({"number":number,"input":fs::read_to_string(self.store.problem(id)?.join(format!("statements/example.{number}.in")))?,"output":fs::read_to_string(self.store.problem(id)?.join(format!("statements/example.{number}.out")))?}));
                    }
                    json!({"manifest":m,"ready":self.store.ready(id)?,"statements":statements,"examples":examples,"tests":self.store.tests(id)?})
                })
            }
            "suite_add" => {
                let suite = s(&v, "suite")?;
                identifier(suite)?;
                let mut m = self.store.manifest(id)?;
                if !m.suites.contains(&suite.to_string()) {
                    m.suites.push(suite.into());
                    self.store.save_manifest(&m)?;
                }
                fs::create_dir_all(self.store.problem(id)?.join(format!("tests/{suite}")))?;
                Ok(json!(m))
            }
            "validator_run" => {
                let m = self.store.manifest(id)?;
                let aid = m.validator.context("validator not configured")?;
                let a = self.store.answer(id, &aid)?;
                let p = self.runtime.prepare(&a.language, &a.source)?;
                let r = self.runtime.run(
                    &p,
                    s(&v, "input")?,
                    &[],
                    m.time_limit_ms,
                    m.memory_limit_mb,
                )?;
                let expect = v["expect_valid"]
                    .as_bool()
                    .context("expect_valid required")?;
                let accepted = r.verdict == "AC";
                let record = json!({"id":uuid(),"validator_id":aid,"revision":a.revision,"input":s(&v,"input")?,"expect_valid":expect,"accepted":accepted,"matched":accepted==expect,"result":r,"created_at":now()});
                write_json(
                    &self.store.problem(id)?.join(format!(
                        "validator-runs/{}.json",
                        record["id"].as_str().unwrap()
                    )),
                    &record,
                )?;
                Ok(record)
            }
            "statement_set" => {
                let section = s(&v, "section")?;
                ensure!(
                    ["legend", "input", "output", "notes"].contains(&section),
                    "unknown statement section"
                );
                self.store.manifest(id)?;
                atomic(
                    &self
                        .store
                        .problem(id)?
                        .join(format!("statements/{section}.md")),
                    s(&v, "markdown")?.as_bytes(),
                )?;
                Ok(json!({"saved":true}))
            }
            "example_set" => {
                let num = n(&v, "number")?;
                ensure!(num > 0 && num <= 1000, "invalid example number");
                let mut m = self.store.manifest(id)?;
                ensure!(
                    num as usize <= m.example_inputs + 1,
                    "examples must be contiguous"
                );
                for (ext, key) in [("in", "input"), ("out", "output")] {
                    atomic(
                        &self
                            .store
                            .problem(id)?
                            .join(format!("statements/example.{num}.{ext}")),
                        s(&v, key)?.as_bytes(),
                    )?;
                }
                m.example_inputs = m.example_inputs.max(num as usize);
                m.example_outputs = m.example_inputs;
                self.store.save_manifest(&m)?;
                Ok(json!(m))
            }
            "settings_update" => {
                let mut m = self.store.manifest(id)?;
                if let Some(name) = v["name"].as_str() {
                    m.name = name.into();
                }
                if let Some(x) = v["time_limit_ms"].as_u64() {
                    ensure!(x > 0 && x <= 300000, "invalid time limit");
                    m.time_limit_ms = x;
                }
                if let Some(x) = v["memory_limit_mb"].as_u64() {
                    ensure!((16..=16384).contains(&x), "invalid memory limit");
                    m.memory_limit_mb = x;
                }
                if let Some(x) = v["checker"].as_str() {
                    ensure!(
                        ["exact", "tokens", "lines"].contains(&x)
                            || x.starts_with("testlib:")
                            || x.starts_with("./evaluate/"),
                        "invalid checker"
                    );
                    m.checker = x.into();
                }
                if let Some(x) = v.get("tags") {
                    m.tags = serde_json::from_value(x.clone())?;
                }
                if let Some(x) = v.get("validator") {
                    m.validator = if x.is_null() {
                        None
                    } else {
                        let a = s(&v, "validator")?;
                        self.store.answer(id, a)?;
                        Some(a.into())
                    };
                }
                self.store.save_manifest(&m)?;
                if m.main_solution.is_some() {
                    self.regenerate(id)?;
                }
                Ok(json!(m))
            }
            "checker_set" => {
                self.store.manifest(id)?;
                atomic(
                    &self.store.problem(id)?.join("evaluate/checker.cpp"),
                    s(&v, "source")?.as_bytes(),
                )?;
                let mut m = self.store.manifest(id)?;
                m.checker = "./evaluate/checker.cpp".into();
                self.store.save_manifest(&m)?;
                Ok(json!(m))
            }
            "answer_save" => {
                let mut m = self.store.manifest(id)?;
                let language = s(&v, "language")?;
                ensure!(
                    self.runtime.config.languages.contains_key(language),
                    "unsupported language"
                );
                if v["main"].as_bool() == Some(true)
                    || v["answer_id"]
                        .as_str()
                        .is_some_and(|a| m.main_solution.as_deref() == Some(a))
                {
                    ensure!(s(&v, "expected")? == "AC", "main solution must expect AC");
                }
                let a = self.store.save_answer(
                    id,
                    v["answer_id"].as_str(),
                    language,
                    s(&v, "source")?,
                    s(&v, "expected")?,
                )?;
                if v["main"].as_bool() == Some(true) {
                    ensure!(a.expected == "AC", "main solution must expect AC");
                    m.main_solution = Some(a.id.clone());
                    self.store.save_manifest(&m)?;
                }
                let regeneration = if m.main_solution.as_ref() == Some(&a.id)
                    || m.validator.as_ref() == Some(&a.id)
                {
                    match self.regenerate(id) {
                        Ok(r) => r,
                        Err(e) => json!({"ready":false,"error":e.to_string()}),
                    }
                } else {
                    Value::Null
                };
                Ok(json!({"answer":a,"regeneration":regeneration}))
            }
            "answers_list" => {
                let p = self.store.problem(id)?.join("answers");
                let mut result = vec![];
                if p.exists() {
                    for e in fs::read_dir(p)? {
                        result.push(read_json::<Answer>(&e?.path().join("current.json"))?);
                    }
                }
                Ok(json!(result))
            }
            "answer_history" => {
                let aid = s(&v, "answer_id")?;
                self.store.answer(id, aid)?;
                let mut records = vec![];
                for e in fs::read_dir(
                    self.store
                        .problem(id)?
                        .join(format!("answers/{aid}/revisions")),
                )? {
                    records.push(read_json::<Answer>(&e?.path())?);
                }
                records.sort_by_key(|a| a.revision);
                Ok(json!(records))
            }
            "test_put" => {
                let t =
                    self.store
                        .put_test(id, s(&v, "suite")?, n(&v, "number")?, s(&v, "input")?)?;
                let generation = if self.store.manifest(id)?.main_solution.is_some() {
                    Some(self.regenerate(id)?)
                } else {
                    None
                };
                Ok(json!({"test":t,"generation":generation}))
            }
            "test_get" => {
                let suite = s(&v, "suite")?;
                let num = n(&v, "number")?;
                let input = fs::read_to_string(self.store.test_path(id, suite, num, "in")?)?;
                let path = self.store.test_path(id, suite, num, "out")?;
                let output = if path.exists() {
                    Some(fs::read_to_string(path)?)
                } else {
                    None
                };
                Ok(
                    json!({"input":input,"output":output,"meta":read_json::<TestMeta>(&self.store.test_path(id,suite,num,"json")?)?,"trust":self.trust(id,&format!("test:{suite}:{num}")).ok()}),
                )
            }
            "generate" => {
                let a = self.store.answer(id, s(&v, "generator_id")?)?;
                let p = self.runtime.prepare(&a.language, &a.source)?;
                let args: Vec<String> =
                    serde_json::from_value(v.get("args").cloned().unwrap_or(json!([])))?;
                let m = self.store.manifest(id)?;
                let r = self
                    .runtime
                    .run(&p, "", &args, m.time_limit_ms, m.memory_limit_mb)?;
                ensure!(r.verdict == "AC", "generator failed: {}", r.stderr);
                let mut t =
                    self.store
                        .put_test(id, s(&v, "suite")?, n(&v, "number")?, &r.stdout)?;
                t.origin = Some(json!({"generator_id":a.id,"revision":a.revision,"args":args}));
                write_json(&self.store.test_path(id, &t.suite, t.number, "json")?, &t)?;
                if m.main_solution.is_some() {
                    self.regenerate(id)?;
                }
                Ok(json!(t))
            }
            "regenerate" => self.regenerate(id),
            "submit" => {
                let source = if let Some(path) = v["file"].as_str() {
                    self.workspace_read(path)?
                } else {
                    s(&v, "source")?.into()
                };
                Ok(json!(self.submit(id, s(&v, "language")?, &source)?))
            }
            "submissions_list" => Ok(json!(self.store.records::<Submission>(id, "submissions")?)),
            "test_run" => {
                let a = self.store.answer(id, s(&v, "answer_id")?)?;
                let submission = self.submit(id, &a.language, &a.source)?;
                Ok(
                    json!({"expected":a.expected,"matched":submission.verdict==a.expected,"submission":submission}),
                )
            }
            "ai_review" => Ok(json!(self.review(
                id,
                s(&v, "target")?,
                s(&v, "provider")?
            )?)),
            "trust_get" => self.trust(id, s(&v, "target")?),
            "reviews_list" => Ok(json!(self.store.records::<Review>(id, "reviews")?)),
            "validator_runs_list" => Ok(json!(self.store.records::<Value>(id, "validator-runs")?)),
            "workspace_list" => {
                let root = self.workspace_root()?;
                let directory = root
                    .join(v["path"].as_str().unwrap_or("."))
                    .canonicalize()?;
                ensure!(directory.starts_with(&root), "directory outside workspace");
                let mut entries = vec![];
                for entry in fs::read_dir(directory)?.take(1000) {
                    let entry = entry?;
                    let kind = entry.file_type()?;
                    if kind.is_symlink() {
                        continue;
                    }
                    entries.push(json!({"path":entry.path().strip_prefix(&root)?.to_string_lossy(),"directory":kind.is_dir(),"bytes":entry.metadata()?.len()}));
                }
                Ok(json!(entries))
            }
            "workspace_read" => {
                Ok(json!({"path":s(&v,"path")?,"content":self.workspace_read(s(&v,"path")?)?}))
            }
            "capabilities" => Ok(
                json!({"languages":self.runtime.config.languages,"checkers":crate::builtin::COMMON_CHECKERS.iter().map(|(id,_)|format!("testlib:{id}")).collect::<Vec<_>>(),"providers":self.runtime.config.providers.keys().collect::<Vec<_>>(),"container":self.runtime.config.container,"workspace":self.runtime.config.workspace_root,"storage_root":self.store.root,"local_memory_enforced":cfg!(target_os="linux"),"version":env!("CARGO_PKG_VERSION")}),
            ),
            _ => bail!("unknown tool {name}"),
        }
    }
    pub fn workspace_root(&self) -> Result<std::path::PathBuf> {
        self.runtime.config.workspace_root.as_ref()
            .context("no workspace selected; start with --workspace <directory>, or submit source directly")?
            .canonicalize().context("workspace is unavailable")
    }
    pub fn workspace_read(&self, path: &str) -> Result<String> {
        let root = self.workspace_root()?;
        let p = root.join(path).canonicalize()?;
        ensure!(p.starts_with(root), "file outside workspace");
        ensure!(fs::metadata(&p)?.len() <= 4 * 1024 * 1024, "file too large");
        Ok(fs::read_to_string(p)?)
    }
}
pub fn render(md: &str) -> String {
    let mut options = pulldown_cmark::Options::empty();
    options.insert(
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_MATH,
    );
    let mut formulas = Vec::new();
    let marker = uuid();
    let events = pulldown_cmark::Parser::new_ext(md, options).map(|event| {
        let (tex, style) = match event {
            pulldown_cmark::Event::InlineMath(ref t) => {
                (t.to_string(), latex2mathml::DisplayStyle::Inline)
            }
            pulldown_cmark::Event::DisplayMath(ref t) => {
                (t.to_string(), latex2mathml::DisplayStyle::Block)
            }
            _ => return event,
        };
        match latex2mathml::latex_to_mathml(&tex, style) {
            Ok(math) => {
                let placeholder = format!("MATH{marker}X{}END", formulas.len());
                let safe_math = ammonia::Builder::default()
                    .tags(
                        [
                            "math",
                            "mi",
                            "mn",
                            "mo",
                            "mrow",
                            "mtext",
                            "mfrac",
                            "msqrt",
                            "mroot",
                            "msub",
                            "msup",
                            "msubsup",
                            "mover",
                            "munder",
                            "munderover",
                            "mspace",
                            "mstyle",
                            "mtable",
                            "mtr",
                            "mtd",
                        ]
                        .into_iter()
                        .collect(),
                    )
                    .tag_attributes(std::collections::HashMap::new())
                    .generic_attributes(
                        [
                            "display",
                            "mathvariant",
                            "accent",
                            "stretchy",
                            "form",
                            "width",
                            "displaystyle",
                            "columnalign",
                            "linethickness",
                        ]
                        .into_iter()
                        .collect(),
                    )
                    .clean(&math)
                    .to_string();
                formulas.push((placeholder.clone(), safe_math));
                pulldown_cmark::Event::Text(placeholder.into())
            }
            Err(_) => pulldown_cmark::Event::Text(format!("${tex}$").into()),
        }
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    let mut clean = ammonia::clean(&html);
    // Renderer output is independently sanitized with a MathML-only allowlist.
    for (placeholder, math) in formulas {
        clean = clean.replace(&placeholder, &math);
    }
    clean
}
pub fn serve(engine: Engine, addr: &str) -> Result<()> {
    serve_with_options(engine, addr, false)
}
pub fn serve_with_options(engine: Engine, addr: &str, open: bool) -> Result<()> {
    let socket: std::net::SocketAddr = addr.parse()?;
    ensure!(socket.ip().is_loopback(), "HTTP must bind loopback");
    let server = tiny_http::Server::http(addr).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let addr = server
        .server_addr()
        .to_ip()
        .context("HTTP address unavailable")?
        .to_string();
    eprintln!("Agentic Judge listening at http://{addr}");
    if open {
        let command = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        if let Err(error) = std::process::Command::new(command)
            .arg(format!("http://{addr}"))
            .spawn()
        {
            eprintln!("Open http://{addr} in a browser ({error})");
        }
    }
    for mut request in server.incoming_requests() {
        // Host validation also prevents DNS rebinding against the local app.
        if !request
            .headers()
            .iter()
            .any(|h| h.field.equiv("Host") && h.value.as_str() == addr)
        {
            let _ = request
                .respond(tiny_http::Response::from_string("invalid host").with_status_code(400));
            continue;
        }
        if request.method() == &tiny_http::Method::Get
            && let Some((body, mime)) = crate::web_ui::asset(request.url())
        {
            let response = tiny_http::Response::from_data(body)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", mime).unwrap())
                    .with_header(tiny_http::Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap())
                    .with_header(tiny_http::Header::from_bytes("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if request.url().starts_with("/api/archive?") {
            let result = (|| -> Result<(Vec<u8>, bool, String)> {
                if let Some(origin) = request.headers().iter().find(|h| h.field.equiv("Origin")) {
                    ensure!(origin.value.as_str() == format!("http://{addr}"), "cross-origin request rejected");
                }
                let url = reqwest::Url::parse(&format!("http://{addr}{}", request.url()))?;
                let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
                let kind = q.get("kind").context("kind required")?;
                let id = q.get("problem_id").context("problem_id required")?;
                let suite = q.get("suite").map(String::as_str).unwrap_or("");
                let number = q.get("number").map(|s| s.parse::<u64>()).transpose()?.unwrap_or(0);
                if request.method() == &tiny_http::Method::Get {
                    let data = engine.export_archive(kind, id, suite, number)?;
                    return Ok((data, true, format!("{id}-{kind}{}{}.zip", if suite.is_empty() { String::new() } else { format!("-{suite}") }, if number == 0 { String::new() } else { format!("-{number}") })));
                }
                ensure!(request.method() == &tiny_http::Method::Post, "use GET or POST");
                ensure!(request.headers().iter().any(|h| h.field.equiv("Content-Type") && h.value.as_str() == "application/zip"), "ZIP content type required");
                let mut bytes = Vec::new();
                request.as_reader().take(crate::archive::LIMIT as u64 + 1).read_to_end(&mut bytes)?;
                let result = engine.import_archive(&bytes, kind, id, suite, number)?;
                Ok((serde_json::to_vec(&result)?, false, String::new()))
            })();
            let response = match result {
                Ok((bytes, download, filename)) => {
                    let mut r = tiny_http::Response::from_data(bytes).with_header(tiny_http::Header::from_bytes("Content-Type", if download { "application/zip" } else { "application/json" }).unwrap());
                    if download { r = r.with_header(tiny_http::Header::from_bytes("Content-Disposition", format!("attachment; filename=\"{filename}\"")).unwrap()); }
                    r
                }
                Err(e) => tiny_http::Response::from_data(json!({"error":e.to_string()}).to_string().into_bytes()).with_status_code(400).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap()),
            }.with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap())
             .with_header(tiny_http::Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap())
             .with_header(tiny_http::Header::from_bytes("Cross-Origin-Resource-Policy", "same-origin").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if request.method() == &tiny_http::Method::Get
            && request.url().starts_with("/api/draft-download?")
        {
            let export = (|| -> Result<(String, String)> {
                if let Some(origin) = request.headers().iter().find(|h| h.field.equiv("Origin")) {
                    ensure!(
                        origin.value.as_str() == format!("http://{addr}"),
                        "cross-origin request rejected"
                    );
                }
                let url = reqwest::Url::parse(&format!("http://{addr}{}", request.url()))?;
                let query: std::collections::HashMap<_, _> =
                    url.query_pairs().into_owned().collect();
                let id = query.get("problem_id").context("problem_id required")?;
                let language = query.get("language").context("language required")?;
                let draft =
                    engine.call("draft_get", json!({"problem_id":id,"language":language}))?;
                let source = draft["source"]
                    .as_str()
                    .context("draft not found")?
                    .to_owned();
                let recipe = engine
                    .runtime
                    .config
                    .languages
                    .get(language)
                    .context("unsupported language")?;
                // Never allow configured filenames to inject a response header or path.
                let filename = if !recipe.filename.is_empty()
                    && recipe
                        .filename
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                {
                    recipe.filename.clone()
                } else {
                    "solution.txt".into()
                };
                Ok((source, filename))
            })();
            let response = match export {
                Ok((source, filename)) => tiny_http::Response::from_string(source)
                    .with_header(
                        tiny_http::Header::from_bytes("Content-Type", "text/plain; charset=utf-8")
                            .unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(
                            "Content-Disposition",
                            format!("attachment; filename=\"{filename}\""),
                        )
                        .unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(
                            "Cross-Origin-Resource-Policy",
                            "same-origin",
                        )
                        .unwrap(),
                    ),
                Err(e) => tiny_http::Response::from_string(e.to_string()).with_status_code(400),
            };
            let _ = request.respond(response);
            continue;
        }
        if request.method() == &tiny_http::Method::Get
            && request.url().starts_with("/api/problems/")
        {
            let asset = (|| -> Result<(Vec<u8>, &'static str)> {
                if let Some(origin) = request.headers().iter().find(|h| h.field.equiv("Origin")) {
                    ensure!(
                        origin.value.as_str() == format!("http://{addr}"),
                        "cross-origin request rejected"
                    );
                }
                let parts: Vec<_> = request.url().split('/').collect();
                ensure!(
                    parts.len() == 6 && parts[4] == "assets",
                    "unknown asset route"
                );
                identifier(parts[3])?;
                let (hash, extension) = parts[5].split_once('.').context("invalid asset name")?;
                ensure!(
                    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                    "invalid asset hash"
                );
                let mime = match extension {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    _ => bail!("unsupported asset type"),
                };
                let root = engine
                    .store
                    .problem(parts[3])?
                    .join("statements/assets")
                    .canonicalize()?;
                ensure!(
                    root.starts_with(&engine.store.root),
                    "asset outside storage"
                );
                let path = root.join(parts[5]).canonicalize()?;
                ensure!(path.starts_with(root), "asset outside storage");
                Ok((fs::read(path)?, mime))
            })();
            let response = match asset {
                Ok((bytes, mime)) => tiny_http::Response::from_data(bytes)
                    .with_header(tiny_http::Header::from_bytes("Content-Type", mime).unwrap())
                    .with_header(
                        tiny_http::Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(
                            "Cross-Origin-Resource-Policy",
                            "same-origin",
                        )
                        .unwrap(),
                    ),
                Err(_) => tiny_http::Response::from_data(b"asset not found".to_vec())
                    .with_status_code(404),
            };
            let _ = request.respond(response);
            continue;
        }
        let response = (|| -> Result<Value> {
            if let Some(origin) = request.headers().iter().find(|h| h.field.equiv("Origin")) {
                ensure!(
                    origin.value.as_str() == format!("http://{addr}"),
                    "cross-origin request rejected"
                );
            }
            ensure!(
                request.method() == &tiny_http::Method::Post,
                "use POST /api/tools/<name>"
            );
            ensure!(
                request
                    .headers()
                    .iter()
                    .any(|h| h.field.equiv("Content-Type")
                        && h.value.as_str().starts_with("application/json")),
                "JSON content type required"
            );
            let name = request
                .url()
                .strip_prefix("/api/tools/")
                .context("unknown route")?
                .to_string();
            let mut bytes = vec![];
            request
                .as_reader()
                .take(4 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 4 * 1024 * 1024, "request too large");
            engine.call(&name, serde_json::from_slice(&bytes)?)
        })();
        let (status, body) = match response {
            Ok(v) => (200, v),
            Err(e) => (400, json!({"error":e.to_string()})),
        };
        let r = tiny_http::Response::from_string(body.to_string())
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                    .unwrap(),
            );
        let _ = request.respond(r);
    }
    Ok(())
}
