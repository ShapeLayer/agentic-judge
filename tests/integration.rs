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
}

#[test]
fn legacy_reviews_keep_unknown_model_identity() {
    let legacy = json!({"id":"legacy","provider":"provider-alias","target":"answer:id","fingerprint":"old","status":"valid","rationale":"legacy evidence","suggested_direction":"","created_at":1});
    let review: Review = serde_json::from_value(legacy).unwrap();
    assert_eq!(review.requested_model, None);
    assert_eq!(review.response_model, None);
}
