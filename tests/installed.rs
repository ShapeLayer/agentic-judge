//! Exercise the copied installation from an unrelated working directory.
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
};

struct Installation {
    _temp: tempfile::TempDir,
    root: PathBuf,
    binary: PathBuf,
    cwd: PathBuf,
}
impl Installation {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let binary = root.join("installed/agentic-judge");
        let cwd = root.join("unrelated folder");
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        fs::create_dir(&cwd).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_agentic-judge"), &binary).unwrap();
        Self {
            _temp: temp,
            root,
            binary,
            cwd,
        }
    }
    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.binary);
        cmd.current_dir(&self.cwd)
            .env("AGENTIC_JUDGE_HOME", self.root.join("user state"));
        cmd
    }
    fn run(&self, args: &[&str]) -> Output {
        let out = self.command().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn call(&self, prefix: &[&str], name: &str, value: Value) -> Value {
        let mut args = vec!["call"];
        args.extend_from_slice(prefix);
        args.push(name);
        let value = value.to_string();
        args.push(&value);
        serde_json::from_slice(&self.run(&args).stdout).unwrap()
    }
    fn local_config(&self) -> PathBuf {
        let path = self.root.join("local.toml");
        fs::write(
            &path,
            "storage_root='data'\nruntime_root='cache/run'\ncontainer=false\n",
        )
        .unwrap();
        path
    }
}
#[test]
fn first_run_is_user_scoped_and_does_not_trust_cwd_config() {
    let app = Installation::new();
    fs::write(
        app.cwd.join("judge.toml"),
        "invalid config from unrelated folder",
    )
    .unwrap();
    let caps = app.call(&[], "capabilities", json!({}));
    assert!(caps["workspace"].is_null());
    assert_eq!(
        caps["storage_root"],
        app.root.join("user state/data").to_str().unwrap()
    );
    assert!(app.root.join("user state/config/judge.toml").is_file());
    assert!(!app.cwd.join("data").exists());
    assert!(!app.cwd.join("run").exists());
    let out = app
        .command()
        .args(["call", "workspace_read", "{\"path\":\"judge.toml\"}"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("no workspace selected"));
    let config = app.root.join("user state/config/judge.toml");
    let before = fs::read(&config).unwrap();
    assert!(
        !app.command()
            .arg("init-config")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(before, fs::read(config).unwrap());
}
#[test]
fn concurrent_first_runs_read_complete_configuration() {
    let app = Installation::new();
    let children: Vec<_> = (0..8)
        .map(|_| {
            app.command()
                .args(["call", "capabilities"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
#[test]
fn external_workspace_and_scratch_submit_and_drafts_survive_restart() {
    let app = Installation::new();
    let config = app.local_config();
    let base = ["--config", config.to_str().unwrap()];
    fs::write(
        app.cwd.join("sum.py"),
        "print(sum(map(int,input().split())))",
    )
    .unwrap();
    app.call(
        &base,
        "problem_create",
        json!({"problem_id":"sum","name":"A+B"}),
    );
    app.call(&base, "answer_save", json!({"problem_id":"sum","language":"python","source":"print(sum(map(int,input().split())))","expected":"AC","main":true}));
    app.call(
        &base,
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    );
    let attached = ["--config", config.to_str().unwrap(), "--workspace", "."];
    assert_eq!(
        app.call(
            &attached,
            "submit",
            json!({"problem_id":"sum","language":"python","file":"sum.py"})
        )["verdict"],
        "AC"
    );
    assert_eq!(
        app.call(
            &base,
            "submit",
            json!({"problem_id":"sum","language":"python","source":"print(5)"})
        )["verdict"],
        "AC"
    );
    app.call(
        &base,
        "draft_save",
        json!({"problem_id":"sum","language":"python","source":"scratch"}),
    );
    app.call(
        &attached,
        "draft_save",
        json!({"problem_id":"sum","language":"python","source":"workspace"}),
    );
    assert_eq!(
        app.call(
            &base,
            "draft_get",
            json!({"problem_id":"sum","language":"python"})
        )["source"],
        "scratch"
    );
    assert_eq!(
        app.call(
            &attached,
            "draft_get",
            json!({"problem_id":"sum","language":"python"})
        )["source"],
        "workspace"
    );
    let second = app.root.join("second workspace");
    fs::create_dir(&second).unwrap();
    assert!(
        app.call(
            &[
                "--config",
                config.to_str().unwrap(),
                "--workspace",
                second.to_str().unwrap()
            ],
            "draft_get",
            json!({"problem_id":"sum","language":"python"})
        )
        .is_null()
    );
    fs::write(app.root.join("secret.txt"), "outside").unwrap();
    std::os::unix::fs::symlink(app.root.join("secret.txt"), app.cwd.join("link.txt")).unwrap();
    for path in ["../secret.txt", "link.txt"] {
        assert!(
            !app.command()
                .args([
                    "call",
                    "--config",
                    config.to_str().unwrap(),
                    "--workspace",
                    ".",
                    "workspace_read",
                    &json!({"path":path}).to_string()
                ])
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    assert_eq!(
        app.call(&base, "submissions_list", json!({"problem_id":"sum"}))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        fs::read_dir(app.root.join("cache/run"))
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn explicit_config_resolves_paths_and_scratch_overrides_workspace() {
    let app = Installation::new();
    let config = app.local_config();
    fs::write(&config,"storage_root='data'\nruntime_root='run'\nworkspace_root='unrelated folder'\ncontainer=false\n").unwrap();
    let caps = app.call(&[config.to_str().unwrap()], "capabilities", json!({}));
    assert_eq!(caps["workspace"], app.cwd.to_str().unwrap());
    assert_eq!(
        caps["storage_root"],
        app.root.join("data").to_str().unwrap()
    );
    assert!(
        app.call(
            &["--config", config.to_str().unwrap(), "--scratch"],
            "capabilities",
            json!({})
        )["workspace"]
            .is_null()
    );
    assert!(
        !app.command()
            .args(["call", "--workspace", ".", "--scratch", "capabilities"])
            .output()
            .unwrap()
            .status
            .success()
    );
}
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[test]
fn installed_http_serves_embedded_app_and_preserves_boundaries() {
    let app = Installation::new();
    let mut server = Server(
        app.command()
            .args(["serve", "--listen", "127.0.0.1:0"])
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(server.0.stderr.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let url = line
        .trim()
        .strip_prefix("Agentic Judge listening at ")
        .unwrap();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    for (path, mime, content) in [
        ("/", "text/html", "풀이 코드"),
        ("/app.js", "text/javascript", "draft_save"),
        ("/app.css", "text/css", "columns"),
        ("/editor.css", "text/css", "monaco-editor"),
        ("/editor.worker.js", "text/javascript", ""),
        ("/tokens.css", "text/css", "--c-bg"),
    ] {
        let response = client.get(format!("{url}{path}")).send().unwrap();
        assert!(response.status().is_success());
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with(mime)
        );
        assert!(response.headers().contains_key("content-security-policy"));
        assert!(response.text().unwrap().contains(content));
    }
    for (tool, args) in [
        (
            "problem_create",
            json!({"problem_id":"export","name":"Export"}),
        ),
        (
            "draft_save",
            json!({"problem_id":"export","language":"python","source":"print('안녕하세요')\n"}),
        ),
    ] {
        assert!(
            client
                .post(format!("{url}/api/tools/{tool}"))
                .json(&args)
                .send()
                .unwrap()
                .status()
                .is_success()
        );
    }
    let archive_url = format!("{url}/api/archive?kind=problem&problem_id=export");
    let response = client.get(&archive_url).send().unwrap();
    assert!(response.status().is_success());
    assert_eq!(response.headers()["content-type"], "application/zip");
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"export-problem.zip\""
    );
    let bytes = response.bytes().unwrap();
    assert_eq!(&bytes[..2], b"PK");
    let import_url = format!("{url}/api/archive?kind=problem&problem_id=restored");
    assert!(
        client
            .post(&import_url)
            .header("Content-Type", "application/zip")
            .body(bytes.to_vec())
            .send()
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(
        client
            .post(&import_url)
            .header("Content-Type", "application/zip")
            .body(bytes.to_vec())
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .get(&archive_url)
            .header("Origin", "https://evil.example")
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .post(&import_url)
            .header("Content-Type", "application/zip")
            .header("Origin", "https://evil.example")
            .body(bytes.to_vec())
            .send()
            .unwrap()
            .status(),
        400
    );
    let download = format!("{url}/api/draft-download?problem_id=export&language=python");
    let response = client.get(&download).send().unwrap();
    assert!(response.status().is_success());
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"main.py\""
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.text().unwrap(), "print('안녕하세요')\n");
    assert_eq!(
        client
            .get(&download)
            .header("Origin", "https://evil.example")
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .get(format!(
                "{url}/api/draft-download?problem_id=missing&language=python"
            ))
            .send()
            .unwrap()
            .status(),
        400
    );
    let endpoint = format!("{url}/api/tools/capabilities");
    assert!(
        client
            .post(&endpoint)
            .header("Origin", url)
            .json(&json!({}))
            .send()
            .unwrap()
            .status()
            .is_success()
    );
    assert_eq!(
        client
            .post(&endpoint)
            .header("Origin", "https://evil.example")
            .json(&json!({}))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .post(&endpoint)
            .header("Host", "evil.example")
            .json(&json!({}))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert!(
        !client
            .get(format!("{url}/src/main.rs"))
            .send()
            .unwrap()
            .status()
            .is_success()
    );
}
#[test]
fn installed_mcp_uses_selected_folder_and_embedded_prompts() {
    let app = Installation::new();
    fs::write(app.cwd.join("source.txt"), "external workspace").unwrap();
    let mut child = app
        .command()
        .args(["mcp", "--workspace", "."])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in [
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"workspace_read","arguments":{"path":"source.txt"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"prompts/get","params":{"name":"register_problem","arguments":{}}}),
    ] {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let values: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        values[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("external workspace")
    );
    assert!(
        values[1]["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("workspace_read")
    );
}
#[test]
fn runtime_install_uses_embedded_recipe_and_checks_loaded_image() {
    use std::os::unix::fs::PermissionsExt;
    let app = Installation::new();
    let fake = app.root.join("podman");
    fs::write(&fake, "#!/bin/sh\ncase \"$1\" in\nbuild) test -f \"$4/Containerfile\" && cp \"$4/Containerfile\" \"$JUDGE_RECIPE_CAPTURE\";;\nimage) test \"$2\" = exists;;\nload) test -f \"$3\";;\npull) test -n \"$2\";;\n*) exit 1;;\nesac\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    let config = app.root.join("runtime.toml");
    fs::write(
        &config,
        format!(
            "storage_root='data'\nruntime_root='run'\ncontainer_command='{}'\n",
            fake.display()
        ),
    )
    .unwrap();
    let recipe = app.root.join("captured");
    let out = app
        .command()
        .args(["runtime-install", "--config", config.to_str().unwrap()])
        .env("JUDGE_RECIPE_CAPTURE", &recipe)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        fs::read_to_string(recipe)
            .unwrap()
            .contains("debian:bookworm-slim")
    );
    assert!(fs::read_dir(app.root.join("run")).unwrap().next().is_none());
    let archive = app.root.join("offline.tar");
    fs::write(&archive, "mock archive").unwrap();
    app.run(&[
        "runtime-install",
        "--config",
        config.to_str().unwrap(),
        "load",
        archive.to_str().unwrap(),
    ]);
    app.run(&[
        "runtime-install",
        "--config",
        config.to_str().unwrap(),
        "pull",
    ]);
    // A tool failure is never reported as installed.
    fs::write(&fake, "#!/bin/sh\nexit 1\n").unwrap();
    assert!(
        !app.command()
            .args([
                "runtime-install",
                "--config",
                config.to_str().unwrap(),
                "load",
                archive.to_str().unwrap()
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn doctor_reports_missing_runtime_without_claiming_success() {
    let app = Installation::new();
    let config = app.root.join("missing-runtime.toml");
    fs::write(&config, "storage_root='data'\nruntime_root='run'\ncontainer_command='/nonexistent/agentic-judge-test-podman'\n").unwrap();
    let output = app
        .command()
        .args(["doctor", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ready"], false);
    assert_eq!(report["checks"][0]["check"], "podman");
    assert_eq!(report["checks"][0]["ok"], false);
}

#[test]
fn local_doctor_checks_configured_commands() {
    let app = Installation::new();
    let config = app.root.join("local-runtime.toml");
    fs::write(&config,"storage_root='data'\nruntime_root='run'\ncontainer=false\n[languages.python]\nfilename='main.py'\ncompile=[]\nrun=['python3','main.py']\n").unwrap();
    let report: Value = serde_json::from_slice(
        &app.run(&["doctor", "--config", config.to_str().unwrap()])
            .stdout,
    )
    .unwrap();
    assert_eq!(report["ready"], true);
    fs::write(&config,"storage_root='data'\nruntime_root='run'\ncontainer=false\n[languages.missing]\nfilename='main'\ncompile=[]\nrun=['/nonexistent/agentic-judge-test-compiler']\n").unwrap();
    assert!(
        !app.command()
            .args(["doctor", "--config", config.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .success()
    );
}
