use agentic_judge::{engine::Engine, model::Config};
use serde_json::json;
use std::fs;

fn setup() -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::new(Config {
        container: false,
        storage_root: dir.path().join("data"),
        runtime_root: dir.path().join("run"),
        workspace_root: Some(dir.path().into()),
        ..Config::default()
    })
    .unwrap();
    (dir, e)
}
const BOJ: &str = "---\nid: 1000\ntitle: A+B\ntime-limit: 0.5 초\nmemory-limit: 128 MB\ntags:\n  - math\n  - arithmetic\n---\n\n## 문제\n\n합 $A+B$를 구한다.\n\n## 예제 출력 1\n\n## 입력\n\n두 정수\n\n## 출력\n\n합\n\n## 예제 입력 1\n\n```\n1 2\n## not a heading\n```\n\n## 예제 출력 1\n\n```\n3\n```\n\n## 힌트\n\n힌트\n\n## 출처\n\n출처\n";
#[test]
fn boj_document_and_duplicate_empty_example() {
    let (dir, e) = setup();
    fs::write(dir.path().join("problem.md"), BOJ).unwrap();
    let r = e
        .call(
            "problem_import",
            json!({"problem_id":"boj-1000","path":"problem.md"}),
        )
        .unwrap();
    assert_eq!(r["manifest"]["time_limit_ms"], 500);
    assert_eq!(r["manifest"]["tags"], json!(["math", "arithmetic"]));
    let p = e
        .call("problem_get", json!({"problem_id":"boj-1000"}))
        .unwrap();
    assert_eq!(p["examples"][0]["input"], "1 2\n## not a heading\n");
    assert_eq!(p["examples"][0]["output"], "3\n");
    assert!(
        p["statements"]["notes"]["markdown"]
            .as_str()
            .unwrap()
            .contains("## 출처")
    );
    assert!(
        p["statements"]["legend"]["html"]
            .as_str()
            .unwrap()
            .contains("<math")
    );
    assert_eq!(p["ready"], false);
    assert_eq!(p["tests"], json!([]));
    assert!(
        e.call(
            "problem_import",
            json!({"problem_id":"boj-1000","path":"problem.md"})
        )
        .is_err()
    );
}
#[test]
fn import_errors_publish_nothing() {
    let (dir, e) = setup();
    fs::write(
        dir.path().join("broken.md"),
        BOJ.replace("0.5 초", "unknown"),
    )
    .unwrap();
    assert!(
        e.call(
            "problem_import",
            json!({"problem_id":"broken","path":"broken.md"})
        )
        .is_err()
    );
    assert!(!e.store.problem("broken").unwrap().exists());
    let outside = tempfile::NamedTempFile::new().unwrap();
    assert!(
        e.call(
            "problem_import",
            json!({"problem_id":"outside","path":outside.path()})
        )
        .is_err()
    );
}
fn polygon_fixture(root: &std::path::Path) {
    fs::create_dir_all(root.join("statements/korean")).unwrap();
    fs::write(root.join("problem.xml"),r#"<problem><names><name language="korean" value="합 &amp; 표"/></names><statements><statement type="text/html" language="korean" path="statements/korean/problem.html"/></statements><judging><testset name="tests"><time-limit>1000</time-limit><memory-limit>268435456</memory-limit></testset></judging><tags><tag value="math"/></tags></problem>"#).unwrap();
    fs::write(root.join("statements/korean/problem.html"),r#"<div class="legend"><p>합 $$$a+b$$$</p><table><tr><td>A</td><td>B</td></tr><tr><td>1</td><td>2</td></tr></table><img src="image.png"></div><div class="input-specification"><div class="section-title">Input</div>두 수</div><div class="output-specification">합</div><div class="note">설명</div>"#).unwrap();
    fs::write(root.join("statements/korean/image.png"), b"image fixture").unwrap();
    fs::write(
        root.join("statements/korean/problem-properties.json"),
        r#"{"sampleTests":[{"inputFile":"example.01","outputFile":"example.01.a"}]}"#,
    )
    .unwrap();
    fs::write(root.join("statements/korean/example.01"), "1 2\n").unwrap();
    fs::write(root.join("statements/korean/example.01.a"), "3\n").unwrap();
}
#[test]
fn polygon_math_tables_images_and_exact_samples() {
    let (dir, e) = setup();
    let root = dir.path().join("package");
    polygon_fixture(&root);
    let r = e
        .call(
            "problem_import",
            json!({"problem_id":"sum","path":"package","language":"korean"}),
        )
        .unwrap();
    assert_eq!(r["manifest"]["name"], "합 & 표");
    assert_eq!(r["manifest"]["memory_limit_mb"], 256);
    assert_eq!(r["manifest"]["tags"], json!(["math"]));
    let p = e.call("problem_get", json!({"problem_id":"sum"})).unwrap();
    let html = p["statements"]["legend"]["html"].as_str().unwrap();
    assert!(html.contains("<math"), "{html}");
    assert!(html.contains("<table>"), "{html}");
    assert!(html.contains("/api/problems/sum/assets/"));
    assert_eq!(p["examples"][0]["input"], "1 2\n");
    assert_eq!(
        fs::read_dir(e.store.problem("sum").unwrap().join("statements/assets"))
            .unwrap()
            .count(),
        1
    );
    assert!(
        e.call(
            "problem_import",
            json!({"problem_id":"missing","path":"package","language":"english"})
        )
        .is_err()
    );
}
#[test]
fn polygon_package_path_traversal_rejected() {
    let (dir, e) = setup();
    let root = dir.path().join("package");
    polygon_fixture(&root);
    let p = root.join("problem.xml");
    let xml = fs::read_to_string(&p).unwrap();
    fs::write(dir.path().join("outside.html"), "secret").unwrap();
    fs::write(
        p,
        xml.replace("statements/korean/problem.html", "../outside.html"),
    )
    .unwrap();
    assert!(
        e.call(
            "problem_import",
            json!({"problem_id":"bad","path":"package"})
        )
        .is_err()
    );
    assert!(!e.store.problem("bad").unwrap().exists());
}

#[test]
fn archive_literal_metadata_annotations_and_incomplete_samples() {
    let (dir, e) = setup();
    let source = BOJ
        .replace("title: A+B", "title: Case: A+B")
        .replace("0.5 초", "0.25 초 (추가 시간 없음)")
        .replace("```\n3\n```", "");
    fs::write(dir.path().join("problem.md"), source).unwrap();
    let r = e
        .call(
            "problem_import",
            json!({"problem_id":"partial","path":"problem.md"}),
        )
        .unwrap();
    assert_eq!(r["manifest"]["name"], "Case: A+B");
    assert_eq!(r["manifest"]["time_limit_ms"], 250);
    assert_eq!(r["manifest"]["example_inputs"], 0);
    assert!(!r["warnings"].as_array().unwrap().is_empty());
    let p = e
        .call("problem_get", json!({"problem_id":"partial"}))
        .unwrap();
    let notes = p["statements"]["notes"]["markdown"].as_str().unwrap();
    assert!(notes.contains("추가 시간 없음"));
    assert!(notes.contains("1 2"));
}
#[test]
fn archive_requires_explicit_unknown_limits() {
    let (dir, e) = setup();
    fs::write(
        dir.path().join("problem.md"),
        BOJ.replace("0.5 초", "서브태스크 참고"),
    )
    .unwrap();
    assert!(
        e.call(
            "problem_import",
            json!({"problem_id":"p","path":"problem.md"})
        )
        .unwrap_err()
        .to_string()
        .contains("override required")
    );
    let r = e
        .call(
            "problem_import",
            json!({"problem_id":"p","path":"problem.md","time_limit_ms":3000}),
        )
        .unwrap();
    assert_eq!(r["manifest"]["time_limit_ms"], 3000);
}
#[test]
fn polygon_html_only_and_escaped_text() {
    let (dir, e) = setup();
    let root = dir.path().join("package");
    polygon_fixture(&root);
    fs::remove_file(root.join("statements/korean/problem-properties.json")).unwrap();
    let path = root.join("statements/korean/problem.html");
    let mut html = fs::read_to_string(&path).unwrap();
    html.push_str(r#"<div class="sample-test"><div class="input"><pre>a<br>b</pre></div><div class="output"><pre>2</pre></div></div>"#);
    html = html.replace("합 $$$a+b$$$", "a_b_c &lt;ok&gt; $$$a_b+c$$$");
    fs::write(path, html).unwrap();
    e.call("problem_import", json!({"problem_id":"p","path":"package"}))
        .unwrap();
    let p = e.call("problem_get", json!({"problem_id":"p"})).unwrap();
    assert_eq!(p["examples"][0]["input"], "a\nb");
    let html = p["statements"]["legend"]["html"].as_str().unwrap();
    assert!(html.contains("a_b_c &lt;ok&gt;"), "{html}");
    assert!(html.contains("<msub>"), "{html}");
}

#[test]
fn http_import_serves_assets_and_rejects_external_origin() {
    let (dir, _) = setup();
    polygon_fixture(&dir.path().join("package"));
    let config = dir.path().join("judge.toml");
    fs::write(
        &config,
        "storage_root='data'\nruntime_root='run'\nworkspace_root='.'\ncontainer=false\n",
    )
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    struct Server(std::process::Child);
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _server = Server(
        std::process::Command::new(env!("CARGO_BIN_EXE_agentic-judge"))
            .arg("serve")
            .arg(config)
            .arg(addr.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap();
    let base = format!("http://{addr}");
    for _ in 0..50 {
        if client.get(&base).send().is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let r = client
        .post(format!("{base}/api/tools/problem_import"))
        .json(&json!({"problem_id":"p","path":"package"}))
        .send()
        .unwrap();
    assert_eq!(r.status(), 200);
    let hash = agentic_judge::model::hash(b"image fixture");
    let url = format!("{base}/api/problems/p/assets/{hash}.png");
    let r = client.get(&url).send().unwrap();
    assert_eq!(r.status(), 200);
    assert_eq!(r.headers()["content-type"], "image/png");
    assert_eq!(r.bytes().unwrap().as_ref(), b"image fixture");
    assert_eq!(
        client
            .get(&url)
            .header("Origin", "https://untrusted.example")
            .send()
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .get(format!("{base}/api/problems/p/assets/manifest.toml"))
            .send()
            .unwrap()
            .status(),
        404
    );
    #[cfg(unix)]
    {
        let outside = dir.path().join("outside.png");
        fs::write(&outside, b"private").unwrap();
        let asset = dir
            .path()
            .join(format!("data/problems/p/statements/assets/{hash}.png"));
        fs::remove_file(&asset).unwrap();
        std::os::unix::fs::symlink(outside, asset).unwrap();
        assert_eq!(client.get(&url).send().unwrap().status(), 404);
    }
}
