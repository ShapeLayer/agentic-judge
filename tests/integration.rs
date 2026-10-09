use agentic_judge::{api, engine::Engine, mcp, model::*, storage::*};
use serde_json::json;
fn setup() -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let c = Config {
        container: false,
        storage_root: dir.path().join("data"),
        runtime_root: dir.path().join("runtime"),
        workspace_root: Some(dir.path().into()),
        ..Config::default()
    };
    let e = Engine::new(c).unwrap();
    e.call("problem_create", json!({"problem_id":"sum","name":"Sum"}))
        .unwrap();
    (dir, e)
}
fn main_solution(e: &Engine) -> String {
    e.call("answer_save",json!({"problem_id":"sum","language":"python","source":"a,b=map(int,input().split()); print(a+b)","expected":"AC","main":true})).unwrap()["answer"]["id"].as_str().unwrap().into()
}
#[test]
fn generation_judging_and_revisions() {
    let (_d, e) = setup();
    let id = main_solution(&e);
    assert!(!e.store.ready("sum").unwrap());
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    assert!(e.store.ready("sum").unwrap());
    for (source, expected) in [
        ("print(sum(map(int,input().split())))", "AC"),
        ("print(0)", "WA"),
        ("raise RuntimeError('panic')", "RE"),
    ] {
        assert_eq!(e.submit("sum", "python", source).unwrap().verdict, expected);
    }
    e.call(
        "settings_update",
        json!({"problem_id":"sum","time_limit_ms":100}),
    )
    .unwrap();
    assert_eq!(
        e.submit("sum", "python", "while True: pass")
            .unwrap()
            .verdict,
        "TLE"
    );
    assert_eq!(
        std::fs::read_dir(&e.runtime.config.runtime_root)
            .unwrap()
            .count(),
        0
    );
    let fp = e.fingerprint("sum", &format!("answer:{id}")).unwrap();
    let review = Review {
        id: uuid(),
        provider: "test".into(),
        requested_model: Some("test-model".into()),
        response_model: None,
        target: format!("answer:{id}"),
        fingerprint: fp,
        status: "valid".into(),
        rationale: "test evidence".into(),
        suggested_direction: "".into(),
        created_at: now(),
    };
    write_json(
        &e.store
            .problem("sum")
            .unwrap()
            .join(format!("reviews/{}.json", review.id)),
        &review,
    )
    .unwrap();
    assert_eq!(e.trust("sum", &review.target).unwrap()["valid"], 1);
    let updated=e.call("answer_save",json!({"problem_id":"sum","answer_id":id,"language":"python","source":"a,b=map(int,input().split()); print(a+b+1)","expected":"AC"})).unwrap();
    assert_eq!(updated["answer"]["revision"], 2);
    assert!(e.store.ready("sum").unwrap());
    assert_eq!(
        std::fs::read_to_string(e.store.test_path("sum", "general", 1, "out").unwrap()).unwrap(),
        "6\n"
    );
    assert_eq!(e.trust("sum", &review.target).unwrap()["valid"], 0);
    assert_eq!(
        e.trust("sum", &review.target).unwrap()["historical_count"],
        1
    );
    assert_eq!(
        e.call("answer_history", json!({"problem_id":"sum","answer_id":id}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    std::fs::write(
        e.store.test_path("sum", "general", 1, "in").unwrap(),
        "8 9\n",
    )
    .unwrap();
    assert!(!e.store.ready("sum").unwrap());
    assert!(e.submit("sum", "python", "print(1)").is_err());
}
#[test]
fn validator_and_failed_main_invalidate() {
    let (_d, e) = setup();
    let main = main_solution(&e);
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    let a=e.call("answer_save",json!({"problem_id":"sum","language":"python","source":"a,b=map(int,input().split()); assert 0<=a<=10 and 0<=b<=10","expected":"AC"})).unwrap();
    e.call(
        "settings_update",
        json!({"problem_id":"sum","validator":a["answer"]["id"]}),
    )
    .unwrap();
    assert!(
        e.call(
            "test_put",
            json!({"problem_id":"sum","suite":"general","number":2,"input":"-1 9\n"})
        )
        .is_err()
    );
    assert!(!e.store.ready("sum").unwrap());
    let r=e.call("answer_save",json!({"problem_id":"sum","answer_id":main,"language":"python","source":"raise RuntimeError('bad main')","expected":"AC"})).unwrap();
    assert_eq!(r["regeneration"]["ready"], false);
}
#[test]
fn workspace_and_identifiers() {
    let (d, e) = setup();
    assert!(e.store.create("../escape", "Bad").is_err());
    assert!(e.workspace_read("/etc/passwd").is_err());
    std::os::unix::fs::symlink("/etc", d.path().join("outside")).unwrap();
    assert!(e.workspace_read("outside/passwd").is_err());
    assert!(e.store.put_test("sum", "../bad", 1, "x").is_err());
    assert!(
        e.store
            .save_answer("sum", Some("not-a-uuid"), "cpp", "", "AC")
            .is_err()
    );
}
#[test]
fn mcp_and_markdown() {
    let (_d, e) = setup();
    let r = mcp::handle(&e, json!({"jsonrpc":"2.0","id":1,"method":"initialize"})).unwrap();
    assert_eq!(r["result"]["protocolVersion"], "2025-11-25");
    assert!(
        mcp::handle(
            &e,
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .is_none()
    );
    let r=mcp::handle(&e,json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"problem_get","arguments":{"problem_id":"missing"}}})).unwrap();
    assert_eq!(r["result"]["isError"], true);
    let html =
        api::render("# Title\n<script>alert(1)</script>\n\n$x^2$\n\n[bad](javascript:alert(1))");
    assert!(!html.contains("<script"));
    assert!(!html.contains("javascript:"));
    assert!(html.contains("<math"));
    assert!(html.contains("<msup>"));
    let malicious = api::render(r"$\text{<img src=x onerror=alert(1)>}$");
    assert!(!malicious.contains("<img"));
    assert!(!malicious.contains("onerror="));
}
#[test]
fn testlib_cpp_checker_and_generator() {
    let (_d, e) = setup();
    main_solution(&e);
    let gen_src = "#include \"testlib.h\"\nint main(int argc,char**argv){registerGen(argc,argv,1); println(rnd.next(1,5),rnd.next(1,5));}";
    let a = e
        .call(
            "answer_save",
            json!({"problem_id":"sum","language":"cpp","source":gen_src,"expected":"AC"}),
        )
        .unwrap();
    e.call("generate",json!({"problem_id":"sum","generator_id":a["answer"]["id"],"suite":"general","number":1,"args":["seed"]})).unwrap();
    e.call("checker_set",json!({"problem_id":"sum","source":"#include \"testlib.h\"\nint main(int argc,char**argv){registerTestlibCmd(argc,argv); long long a=ans.readLong(),b=ouf.readLong(); if(a!=b)quitf(_wa,\"wrong\");quitf(_ok,\"ok\");}"})).unwrap();
    assert_eq!(
        e.submit("sum", "python", "print(sum(map(int,input().split())))")
            .unwrap()
            .verdict,
        "AC"
    );
    assert_eq!(
        e.submit("sum", "python", "print(99)").unwrap().verdict,
        "WA"
    );
    e.call(
        "settings_update",
        json!({"problem_id":"sum","checker":"testlib:ncmp"}),
    )
    .unwrap();
    assert_eq!(
        e.submit("sum", "python", "print(sum(map(int,input().split())))")
            .unwrap()
            .verdict,
        "AC"
    );
}

#[test]
fn ai_reviews_record_failures_and_never_modify_answers() {
    let (d, mut e) = setup();
    let aid = main_solution(&e);
    let original = e.store.answer("sum", &aid).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        use std::io::{Read, Write};
        for response in [
            json!({"model":"mock-resolved-v1","choices":[{"message":{"content":"{\"status\":\"invalid\",\"rationale\":\"counterexample\",\"suggested_direction\":\"check boundary\"}"}}]}),
            json!({"model":"mock-resolved-v2","choices":[{"message":{"content":"not JSON"}}]}),
            json!({"choices":[{"message":{"content":"{\"status\":\"valid\",\"rationale\":\"ok\",\"suggested_direction\":\"\"}"}}]}),
        ] {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut buf = vec![];
            let mut byte = [0];
            while !buf.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                buf.push(byte[0]);
            }
            let headers = String::from_utf8(buf).unwrap();
            let length: usize = headers
                .lines()
                .find_map(|l| {
                    l.to_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_string)
                })
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert!(
                request["messages"][0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("Never edit")
            );
            let body = response.to_string();
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        }
    });
    e.runtime.config.providers.insert(
        "mock".into(),
        Provider {
            kind: "openai-compatible".into(),
            endpoint: format!("http://{addr}"),
            model: "mock".into(),
            api_key_env: None,
        },
    );
    let target = format!("answer:{aid}");
    let first = e.review("sum", &target, "mock").unwrap();
    assert_eq!(first.status, "invalid");
    assert_eq!(first.requested_model.as_deref(), Some("mock"));
    assert_eq!(first.response_model.as_deref(), Some("mock-resolved-v1"));
    let second = e.review("sum", &target, "mock").unwrap();
    assert_eq!(second.status, "error");
    assert_eq!(second.requested_model.as_deref(), Some("mock"));
    assert_eq!(second.response_model.as_deref(), Some("mock-resolved-v2"));
    let third = e.review("sum", &target, "mock").unwrap();
    assert_eq!(third.status, "valid");
    assert_eq!(third.requested_model.as_deref(), Some("mock"));
    assert_eq!(third.response_model, None);
    let records: Vec<Review> = e.store.records("sum", "reviews").unwrap();
    assert!(
        records
            .iter()
            .all(|r| r.requested_model.as_deref() == Some("mock"))
    );
    assert!(
        records
            .iter()
            .any(|r| r.response_model.as_deref() == Some("mock-resolved-v2"))
    );
    server.join().unwrap();
    assert_eq!(e.store.answer("sum", &aid).unwrap().source, original.source);
    assert_eq!(e.store.answer("sum", &aid).unwrap().revision, 1);
    assert_eq!(e.trust("sum", &target).unwrap()["invalid"], 1);
    assert_eq!(e.trust("sum", &target).unwrap()["errors"], 1);
    assert!(d.path().join("data").exists());
    let provider = e.runtime.config.providers.get_mut("mock").unwrap();
    provider.model = "changed-model".into();
    provider.endpoint = "invalid-url".into();
    let failed = e.review("sum", &target, "mock").unwrap();
    assert_eq!(failed.status, "error");
    assert_eq!(failed.requested_model.as_deref(), Some("changed-model"));
    assert_eq!(failed.response_model, None);
    let persisted = e.call("reviews_list", json!({"problem_id":"sum"})).unwrap();
    let old = persisted
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == first.id)
        .unwrap();
    assert_eq!(old["requested_model"], "mock");
    assert_eq!(old["response_model"], "mock-resolved-v1");
}

#[test]
fn isolated_test_runs_and_validator_negative_cases() {
    let (_d, e) = setup();
    main_solution(&e);
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":2,"input":"4 5\n"}),
    )
    .unwrap();
    let src = "from pathlib import Path\nassert not Path('marker').exists()\nPath('marker').write_text('x')\nprint(sum(map(int,input().split())))";
    assert_eq!(e.submit("sum", "python", src).unwrap().verdict, "AC");
    let validator=e.call("answer_save",json!({"problem_id":"sum","language":"python","source":"a,b=map(int,input().split()); assert a>=0 and b>=0","expected":"AC"})).unwrap();
    e.call(
        "settings_update",
        json!({"problem_id":"sum","validator":validator["answer"]["id"]}),
    )
    .unwrap();
    let r = e
        .call(
            "validator_run",
            json!({"problem_id":"sum","input":"-1 3\n","expect_valid":false}),
        )
        .unwrap();
    assert_eq!(r["matched"], true);
    assert!(
        e.runtime
            .config
            .runtime_root
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}

#[test]
fn stdio_mcp_end_to_end() {
    use std::io::Write;
    let (dir, _e) = setup();
    let config = dir.path().join("judge.toml");
    std::fs::write(
        &config,
        "storage_root='data'\nruntime_root='runtime'\nworkspace_root='.'\ncontainer=false\n",
    )
    .unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_agentic-judge"))
        .arg("mcp")
        .arg(&config)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for request in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"problems_list","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"prompts/get","params":{"name":"register_problem","arguments":{"context":"sum"}}}),
    ] {
        writeln!(stdin, "{request}").unwrap();
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let lines: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1]["result"]["isError"], false);
    assert!(
        lines[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("sum")
    );
    assert!(
        lines[2]["result"]["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("workspace_read")
    );
}

#[test]
#[cfg(target_os = "macos")]
fn local_peak_memory_limit_is_reported() {
    let (_dir, e) = setup();
    let p = e
        .runtime
        .prepare("python", "x=bytearray(96*1024*1024); print(len(x))")
        .unwrap();
    let r = e.runtime.run(&p, "", &[], 5000, 64).unwrap();
    assert_eq!(r.verdict, "MLE");
    assert!(r.memory_kb > 64 * 1024);
}

#[test]
fn http_api_end_to_end_and_origin_boundary() {
    let (dir, _e) = setup();
    let config = dir.path().join("judge.toml");
    std::fs::write(
        &config,
        "storage_root='data'\nruntime_root='runtime'\nworkspace_root='.'\ncontainer=false\n",
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
    let url = format!("http://{addr}/api/tools/problems_list");
    let mut ready = false;
    for _ in 0..50 {
        if client.get(&url).send().is_ok() {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(ready);
    let response = client.post(&url).json(&json!({})).send().unwrap();
    assert_eq!(response.status(), 200);
    let problems: serde_json::Value = response.json().unwrap();
    assert_eq!(problems[0]["id"], "sum");
    assert_eq!(
        client
            .post(&url)
            .header("Origin", "https://untrusted.example")
            .json(&json!({}))
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(client.post(&url).body("{}").send().unwrap().status(), 400);
    let record_path = _e
        .store
        .problem("sum")
        .unwrap()
        .join("submissions/download.json");
    let original = b"{\"source\":\"original\",\"cases\":[]}";
    std::fs::create_dir_all(record_path.parent().unwrap()).unwrap();
    std::fs::write(&record_path, original).unwrap();
    let download_url =
        format!("http://{addr}/api/submission-download?problem_id=sum&submission_id=download");
    let response = client.get(&download_url).send().unwrap();
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    assert_eq!(response.bytes().unwrap().as_ref(), original);
    assert_eq!(
        client
            .get(&download_url)
            .header("Origin", "https://untrusted.example")
            .send()
            .unwrap()
            .status(),
        400
    );
    assert_eq!(
        client
            .get(format!(
                "http://{addr}/api/submission-download?problem_id=sum&submission_id=..%2Fdownload"
            ))
            .send()
            .unwrap()
            .status(),
        400
    );
}

#[test]
fn legacy_reviews_keep_unknown_model_identity() {
    let legacy = json!({"id":"legacy","provider":"provider-alias","target":"answer:id","fingerprint":"old","status":"valid","rationale":"legacy evidence","suggested_direction":"","created_at":1});
    let review: Review = serde_json::from_value(legacy).unwrap();
    assert_eq!(review.requested_model, None);
    assert_eq!(review.response_model, None);
}

#[test]
fn searchable_catalog_pages_and_external_updates() {
    let (_dir, e) = setup();
    for i in 0..125 {
        e.store
            .create(&format!("boj-{i:04}"), &format!("연습 {i}"))
            .unwrap();
    }
    let first = e
        .call("problems_search", json!({"query":"연습", "limit":50}))
        .unwrap();
    assert_eq!(first["total"], 125);
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    let last = e
        .call(
            "problems_search",
            json!({"query":"연습", "offset":100, "limit":50}),
        )
        .unwrap();
    assert_eq!(last["items"].as_array().unwrap().len(), 25);
    assert_eq!(last["items"][0]["id"], "boj-0100");
    let other = Store::new(&e.store.root).unwrap();
    other.create("external", "다른 세션의 문제").unwrap();
    let mut m = other.manifest("external").unwrap();
    m.tags = vec!["그래프".into()];
    other.save_manifest(&m).unwrap();
    let found = e
        .call("problems_search", json!({"query":"그래프"}))
        .unwrap();
    assert_eq!(found["total"], 1);
    assert_eq!(found["items"][0]["id"], "external");
    m.name = "변경된 제목입니다".into();
    other.save_manifest(&m).unwrap();
    assert_eq!(
        e.call("problems_search", json!({"query":"변경된"}))
            .unwrap()["total"],
        1
    );
    std::fs::remove_dir_all(other.problem("external").unwrap()).unwrap();
    assert_eq!(
        e.call("problems_search", json!({"query":"그래프"}))
            .unwrap()["total"],
        0
    );
    assert_eq!(
        e.call("problems_search", json!({"limit":1000})).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        100
    );
}

#[test]
fn legacy_records_compact_and_data_is_loaded_only_by_request() {
    let (_d, e) = setup();
    let record_id = uuid();
    let input = "가나다\n".repeat(200_000);
    let output = "5\n".repeat(100_000);
    let legacy = json!({"id":record_id,"problem_id":"sum","language":"python","source":"print(5)","created_at":1,"generation":"test","verdict":"AC","cases":[{"suite":"general","number":1,"input":input,"expected_output":output,"checker_message":"OK","result":{"verdict":"AC","time_ms":10,"memory_kb":1,"stdout":output,"stderr":"","exit_code":0}}]});
    let path = e
        .store
        .problem("sum")
        .unwrap()
        .join(format!("submissions/{record_id}.json"));
    write_json(&path, &legacy).unwrap();
    e.store.put_test("sum", "general", 1, &input).unwrap();
    std::fs::write(
        e.store.test_path("sum", "general", 1, "out").unwrap(),
        &output,
    )
    .unwrap();
    let compact = e
        .call("submissions_compact", json!({"problem_id":"sum"}))
        .unwrap();
    assert!(compact["record_bytes_before"].as_u64().unwrap() > 1_000_000);
    assert!(compact["record_bytes_after"].as_u64().unwrap() < 2048);
    let persisted: serde_json::Value = read_json(&path).unwrap();
    assert!(persisted["cases"][0].get("input").is_none());
    assert!(persisted["cases"][0].get("expected_output").is_none());
    assert!(persisted["cases"][0]["result"].get("stdout").is_none());
    assert_eq!(persisted["source"], "print(5)");
    assert_eq!(persisted["verdict"], "AC");
    let preview = e
        .call(
            "submission_preview",
            json!({"problem_id":"sum","submission_id":record_id}),
        )
        .unwrap();
    assert!(preview.to_string().len() < 1024);
    assert!(preview["cases"][0].get("input").is_none());
    let data = e
        .call(
            "submission_data_preview",
            json!({"problem_id":"sum","submission_id":record_id,"case_index":0,"field":"input"}),
        )
        .unwrap();
    assert_eq!(data["bytes"], input.len());
    assert_eq!(data["text"].as_str().unwrap().chars().count(), 4096);
    assert_eq!(data["truncated"], true);
    assert_eq!(
        data["path"],
        e.store
            .test_path("sum", "general", 1, "in")
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    let stdout_args =
        json!({"problem_id":"sum","submission_id":record_id,"case_index":0,"field":"stdout"});
    let actual = e
        .call("submission_data_preview", stdout_args.clone())
        .unwrap();
    assert_eq!(actual["bytes"], output.len());
    assert_eq!(
        std::fs::read_to_string(actual["path"].as_str().unwrap()).unwrap(),
        output
    );
    e.call("submissions_compact", json!({"problem_id":"sum"}))
        .unwrap();
    assert_eq!(
        e.call("submission_data_preview", stdout_args.clone())
            .unwrap()["bytes"],
        output.len()
    );
    std::fs::remove_file(actual["path"].as_str().unwrap()).unwrap();
    assert_eq!(
        e.call("submission_data_preview", stdout_args.clone())
            .unwrap()["exists"],
        false
    );
    e.call("submissions_compact", json!({"problem_id":"sum"}))
        .unwrap();
    assert_eq!(
        e.call("submission_data_preview", stdout_args).unwrap()["exists"],
        false
    );
    assert_eq!(
        e.call(
            "submission_preview",
            json!({"problem_id":"sum","submission_id":record_id})
        )
        .unwrap()["cases"][0]["verdict"],
        "AC"
    );
    assert!(e.call("submission_data_preview",json!({"problem_id":"sum","submission_id":record_id,"case_index":0,"field":"../outside"})).is_err());
    assert!(
        e.call(
            "submission_preview",
            json!({"problem_id":"sum","submission_id":"../outside"})
        )
        .is_err()
    );
    let outside = e.store.root.parent().unwrap().join("outside.txt");
    std::fs::write(&outside, "secret").unwrap();
    let diagnostic = e
        .store
        .problem("sum")
        .unwrap()
        .join(format!("submission-data/{record_id}/0/stderr.txt"));
    std::fs::remove_file(&diagnostic).unwrap();
    std::os::unix::fs::symlink(outside, &diagnostic).unwrap();
    assert!(
        e.call(
            "submission_data_preview",
            json!({"problem_id":"sum","submission_id":record_id,"case_index":0,"field":"stderr"})
        )
        .is_err()
    );
}

#[test]
fn ui_submit_returns_only_summary_and_keeps_full_record() {
    let (_d, e) = setup();
    main_solution(&e);
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    let result = e
        .call(
            "submit",
            json!({"problem_id":"sum","language":"python","source":"print(5)","summary_only":true}),
        )
        .unwrap();
    assert_eq!(result["verdict"], "AC");
    assert!(result.get("cases").is_none());
    assert!(result.get("source").is_none());
    let record: Submission = read_json(
        &e.store
            .submission_path("sum", result["id"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(record.source, "print(5)");
    assert!(record.cases[0].input.is_empty());
    assert!(record.cases[0].result.stdout.is_empty());
    assert_eq!(record.cases[0].input_hash, Some(hash(b"2 3\n")));
    let data = e
        .call(
            "submission_data_preview",
            json!({"problem_id":"sum","submission_id":record.id,"case_index":0,"field":"stdout"}),
        )
        .unwrap();
    assert_eq!(data["text"], "5\n");
    assert_eq!(
        std::fs::read_dir(&e.runtime.config.runtime_root)
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn global_submission_history_filters_pages_and_external_records() {
    let (_d, e) = setup();
    e.store.create("other", "Other Problem").unwrap();
    e.store.create("no-submissions", "Empty").unwrap();
    for i in 0..45 {
        let record = Submission {
            purpose: SubmissionPurpose::Solve,
            preparation: None,
            format_version: 2,
            id: format!("record-{i:03}"),
            problem_id: if i % 2 == 0 { "sum" } else { "other" }.into(),
            language: if i % 2 == 0 { "cpp" } else { "python" }.into(),
            source: "code".into(),
            created_at: i,
            generation: "test".into(),
            verdict: if i % 3 == 0 { "AC" } else { "WA" }.into(),
            cases: vec![],
        };
        write_json(
            &e.store
                .problem(&record.problem_id)
                .unwrap()
                .join(format!("submissions/{}.json", record.id)),
            &record,
        )
        .unwrap();
    }
    let all = e.call("submissions_page", json!({})).unwrap();
    assert_eq!(all["total"], 45);
    assert_eq!(all["items"].as_array().unwrap().len(), 20);
    assert_eq!(all["items"][0]["id"], "record-044");
    assert_eq!(all["items"][0]["problem_name"], "Sum");
    let last = e.call("submissions_page", json!({"offset":40})).unwrap();
    assert_eq!(last["items"].as_array().unwrap().len(), 5);
    assert_eq!(last["items"][4]["id"], "record-000");
    let filtered = e
        .call(
            "submissions_page",
            json!({"query":"OTHER", "language":"python", "verdict":"AC"}),
        )
        .unwrap();
    assert_eq!(filtered["total"], 7);
    assert!(
        filtered["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["problem_id"] == "other"
                && s["verdict"] == "AC"
                && s["language"] == "python")
    );
    assert_eq!(
        e.call("submissions_page", json!({"problem_id":"sum"}))
            .unwrap()["total"],
        23
    );
    assert_eq!(
        e.call("submissions_page", json!({"query":"no-submissions"}))
            .unwrap()["total"],
        0
    );
    let other_session = Store::new(&e.store.root).unwrap();
    let mut m = other_session.manifest("other").unwrap();
    m.name = "Renamed Problem".into();
    other_session.save_manifest(&m).unwrap();
    assert_eq!(
        e.call("submissions_page", json!({"query":"renamed"}))
            .unwrap()["total"],
        22
    );
    let record: Submission = read_json(
        &other_session
            .submission_path("other", "record-043")
            .unwrap(),
    )
    .unwrap();
    let new = Submission {
        format_version: 2,
        id: "new-session-record".into(),
        created_at: 100,
        ..record
    };
    write_json(
        &other_session
            .problem("other")
            .unwrap()
            .join("submissions/new-session-record.json"),
        &new,
    )
    .unwrap();
    let updated = e.call("submissions_page", json!({})).unwrap();
    assert_eq!(updated["total"], 46);
    assert_eq!(updated["items"][0]["id"], "new-session-record");
}

#[test]
fn compile_errors_keep_diagnostics_in_files_and_remove_builds() {
    let (_d, e) = setup();
    main_solution(&e);
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    let result = e.submit("sum", "cpp", "this is invalid C++").unwrap();
    assert_eq!(result.verdict, "CE");
    assert_eq!(
        std::fs::read_dir(&e.runtime.config.runtime_root)
            .unwrap()
            .count(),
        0
    );
    let diagnostic = e
        .call(
            "submission_data_preview",
            json!({"problem_id":"sum","submission_id":result.id,"case_index":0,"field":"stderr"}),
        )
        .unwrap();
    assert!(diagnostic["bytes"].as_u64().unwrap() > 0);
    assert!(diagnostic["exists"].as_bool().unwrap());
    let record: serde_json::Value =
        read_json(&e.store.submission_path("sum", &result.id).unwrap()).unwrap();
    assert!(record["cases"][0]["result"].get("stderr").is_none());
}

#[test]
fn async_submission_returns_pending_then_persists_verdict() {
    let (_d, e) = setup();
    main_solution(&e);
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    let result = e.call("submit", json!({"problem_id":"sum","language":"python","source":"import time; time.sleep(0.3); print(5)","summary_only":true,"async":true})).unwrap();
    assert_eq!(result["verdict"], "JUDGING");
    assert!(result.get("source").is_none());
    let path = e
        .store
        .submission_path("sum", result["id"].as_str().unwrap())
        .unwrap();
    let pending: Submission = read_json(&path).unwrap();
    assert_eq!(pending.verdict, "JUDGING");
    assert!(pending.cases.is_empty());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let record: Submission = read_json(&path).unwrap();
        if record.verdict != "JUDGING" {
            assert_eq!(record.verdict, "AC");
            // The summary is saved immediately after the full record.
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "judging did not finish"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn artifact_model_provenance_survives_regeneration_and_revisions() {
    let (_d, e) = setup();
    let saved = e.call("answer_save", json!({"problem_id":"sum","language":"python","source":"print(sum(map(int,input().split())))","expected":"AC","main":true,"generated_by_model":"creator-a"})).unwrap();
    let aid = saved["answer"]["id"].as_str().unwrap();
    e.call("test_put", json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n","generated_by_model":"creator-t"})).unwrap();
    let target = format!("test:general:1");
    let review = Review {
        id: uuid(),
        provider: "reviewer".into(),
        requested_model: Some("requested".into()),
        response_model: Some("actual".into()),
        target: target.clone(),
        fingerprint: e.fingerprint("sum", &target).unwrap(),
        status: "valid".into(),
        rationale: "checked".into(),
        suggested_direction: String::new(),
        created_at: now(),
    };
    write_json(
        &e.store
            .problem("sum")
            .unwrap()
            .join(format!("reviews/{}.json", review.id)),
        &review,
    )
    .unwrap();
    let items = e
        .call("artifact_models", json!({"problem_id":"sum"}))
        .unwrap();
    assert_eq!(items[0]["generated_by_model"], "creator-a");
    assert_eq!(items[1]["generated_by_model"], "creator-t");
    assert_eq!(items[1]["trust"]["reviews"][0]["response_model"], "actual");
    e.call("regenerate", json!({"problem_id":"sum"})).unwrap();
    assert_eq!(
        e.store.tests("sum").unwrap()[0]
            .generated_by_model
            .as_deref(),
        Some("creator-t")
    );
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"4 5\n"}),
    )
    .unwrap();
    let items = e
        .call("artifact_models", json!({"problem_id":"sum"}))
        .unwrap();
    assert!(items[1]["generated_by_model"].is_null());
    assert_eq!(items[1]["trust"]["historical_count"], 1);
    assert!(items[1]["trust"]["reviews"].as_array().unwrap().is_empty());
    e.call("answer_save", json!({"problem_id":"sum","answer_id":aid,"language":"python","source":"print(sum(map(int,input().split())))","expected":"AC","generated_by_model":"creator-b"})).unwrap();
    let history = e
        .call(
            "answer_history",
            json!({"problem_id":"sum","answer_id":aid}),
        )
        .unwrap();
    assert_eq!(history[0]["generated_by_model"], "creator-a");
    assert_eq!(history[1]["generated_by_model"], "creator-b");
}

#[test]
fn preparation_runs_are_separate_and_classification_is_reversible() {
    let (_dir, e) = setup();
    main_solution(&e);
    e.call(
        "test_put",
        json!({"problem_id":"sum","suite":"general","number":1,"input":"2 3\n"}),
    )
    .unwrap();
    let solve = e.submit("sum", "python", "print(0)").unwrap();
    let wrong = e
        .call(
            "answer_save",
            json!({"problem_id":"sum","language":"python","source":"print(0)","expected":"WA"}),
        )
        .unwrap();
    let verification = e
        .call(
            "test_run",
            json!({"problem_id":"sum","answer_id":wrong["answer"]["id"]}),
        )
        .unwrap();
    assert_eq!(verification["matched"], true);
    assert_eq!(verification["submission"]["purpose"], "preparation");
    assert_eq!(verification["submission"]["preparation"]["expected"], "WA");
    assert_eq!(verification["submission"]["preparation"]["revision"], 1);
    let page = e
        .call("submissions_page", json!({"problem_id":"sum"}))
        .unwrap();
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["id"], solve.id);
    assert_eq!(e.call("submissions_page", json!({})).unwrap()["total"], 1);
    let prep = e
        .call(
            "submissions_page",
            json!({"problem_id":"sum","purpose":"preparation","verdict":"WA"}),
        )
        .unwrap();
    assert_eq!(prep["total"], 1);
    assert_eq!(prep["items"][0]["preparation"]["expected"], "WA");
    assert_eq!(
        e.call("submissions_page", json!({"purpose":"all"}))
            .unwrap()["total"],
        2
    );
    assert!(
        e.call("submissions_page", json!({"purpose":"invalid"}))
            .is_err()
    );
    e.call("submission_classify",json!({"problem_id":"sum","submission_id":solve.id,"purpose":"preparation","name":"old check"})).unwrap();
    assert_eq!(e.call("submissions_page", json!({})).unwrap()["total"], 0);
    assert_eq!(
        e.call("submissions_page", json!({"purpose":"preparation"}))
            .unwrap()["total"],
        2
    );
    let original: Submission =
        read_json(&e.store.submission_path("sum", &solve.id).unwrap()).unwrap();
    assert_eq!(original.source, "print(0)");
    assert_eq!(original.cases.len(), 1);
    e.call(
        "submission_classify",
        json!({"problem_id":"sum","submission_id":solve.id,"purpose":"solve"}),
    )
    .unwrap();
    assert_eq!(e.call("submissions_page", json!({})).unwrap()["total"], 1);
    let pending=e.call("submit",json!({"problem_id":"sum","language":"python","source":"print(0)","purpose":"preparation","expected":"WA","name":"async wrong","async":true,"summary_only":true})).unwrap();
    assert_eq!(pending["purpose"], "preparation");
    assert_eq!(pending["preparation"]["expected"], "WA");
    let mut finished = false;
    for _ in 0..100 {
        let record: Submission = read_json(
            &e.store
                .submission_path("sum", pending["id"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        if record.verdict != "JUDGING" {
            assert_eq!(record.purpose, SubmissionPurpose::Preparation);
            finished = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(finished);
}

#[test]
fn legacy_submission_defaults_to_solving() {
    let value = json!({"id":"legacy","problem_id":"sum","language":"python","source":"print(0)","created_at":1,"generation":"g","verdict":"WA","cases":[]});
    let record: Submission = serde_json::from_value(value.clone()).unwrap();
    let summary: SubmissionSummary = serde_json::from_value(value).unwrap();
    assert_eq!(record.purpose, SubmissionPurpose::Solve);
    assert_eq!(summary.purpose, SubmissionPurpose::Solve);
    assert!(record.preparation.is_none());
}

#[test]
fn submission_page_metrics_upgrade_only_visible_records_without_large_payloads() {
    let (_dir, e) = setup();
    let source = "print('한글')";
    for (id, created_at) in [("older", 1), ("newer", 2)] {
        let record = json!({"id":id,"problem_id":"sum","language":"python","source":source,"created_at":created_at,"generation":"g","verdict":"WA","cases":[
            {"suite":"general","number":1,"result":{"verdict":"AC","time_ms":7,"memory_kb":1024,"stdout":"x".repeat(1_000_000),"exit_code":0}},
            {"suite":"general","number":2,"result":{"verdict":"WA","time_ms":19,"memory_kb":4096,"exit_code":0}},
            {"suite":"","number":0,"result":{"verdict":"CE","time_ms":0,"memory_kb":0,"exit_code":1}}
        ]});
        write_json(
            &e.store
                .problem("sum")
                .unwrap()
                .join(format!("submissions/{id}.json")),
            &record,
        )
        .unwrap();
    }
    let page = e
        .call("submissions_page", json!({"problem_id":"sum","limit":1}))
        .unwrap();
    let item = &page["items"][0];
    assert_eq!(item["id"], "newer");
    assert_eq!(item["source_bytes"], source.len());
    assert_eq!(item["time_ms"], 19);
    assert_eq!(item["memory_kb"], 4096);
    assert_eq!(item["case_count"], 2);
    assert_eq!(item["passed_count"], 1);
    assert!(item.get("source").is_none());
    assert!(item.get("cases").is_none());
    assert!(serde_json::to_vec(&page).unwrap().len() < 1500);
    let cache: SubmissionSummary = read_json(
        &e.store
            .problem("sum")
            .unwrap()
            .join("submission-summaries/older.json"),
    )
    .unwrap();
    assert!(cache.source_bytes.is_none());
    let again = e
        .call("submissions_page", json!({"problem_id":"sum","limit":1}))
        .unwrap();
    assert_eq!(page, again);
    let second = e
        .call(
            "submissions_page",
            json!({"problem_id":"sum","limit":1,"offset":1}),
        )
        .unwrap();
    assert_eq!(second["items"][0]["source_bytes"], source.len());
}

#[test]
fn artifact_models_include_non_main_answers() {
    let (_d, e) = setup();
    main_solution(&e);
    let saved = e.call("answer_save", json!({"problem_id":"sum","language":"python","source":"print(0)","expected":"WA","generated_by_model":"other-creator"})).unwrap();
    let target = format!("answer:{}", saved["answer"]["id"].as_str().unwrap());
    let items = e
        .call("artifact_models", json!({"problem_id":"sum"}))
        .unwrap();
    let item = items
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["target"] == target)
        .unwrap();
    assert_eq!(item["generated_by_model"], "other-creator");
    assert_eq!(item["label"], "풀이 코드");
    assert!(item["trust"]["reviews"].as_array().unwrap().is_empty());
}
