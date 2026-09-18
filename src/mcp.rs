use crate::engine::Engine;
use anyhow::Result;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub fn tools() -> Vec<Value> {
    let definitions = [
        (
            "validator_runs_list",
            "Read persisted validator positive/negative runs",
            "problem_id",
        ),
        (
            "workspace_list",
            "Discover source files and directories inside the configured workspace",
            "?path",
        ),
        ("suite_add", "Create a named test suite", "problem_id suite"),
        (
            "validator_run",
            "Run the configured validator on valid or intentionally invalid input and record expected acceptance",
            "problem_id input expect_valid",
        ),
        ("problems_list", "List local problems", ""),
        (
            "problem_create",
            "Create a problem with a general test suite",
            "problem_id name",
        ),
        (
            "problem_get",
            "Read statement HTML/Markdown, settings and readiness",
            "problem_id",
        ),
        (
            "statement_set",
            "Write one Markdown statement section",
            "problem_id section markdown",
        ),
        (
            "example_set",
            "Set a contiguous example input/output pair",
            "problem_id number input output",
        ),
        (
            "settings_update",
            "Update limits, checker, tags or validator",
            "problem_id ?name ?time_limit_ms ?memory_limit_mb ?checker ?tags ?validator",
        ),
        (
            "checker_set",
            "Install a C++ testlib checker at ./evaluate/checker.cpp",
            "problem_id source",
        ),
        (
            "answer_save",
            "Create or revise a UUID answer, validator or generator; main changes regenerate outputs",
            "problem_id language source expected ?answer_id ?main",
        ),
        ("answers_list", "List current UUID answers", "problem_id"),
        (
            "answer_history",
            "Read immutable answer revisions",
            "problem_id answer_id",
        ),
        (
            "test_put",
            "Save input and regenerate main-solution outputs",
            "problem_id suite number input",
        ),
        (
            "test_get",
            "Read a test pair and current trust",
            "problem_id suite number",
        ),
        (
            "generate",
            "Run a saved generator with argv and save its output as input",
            "problem_id generator_id suite number ?args",
        ),
        (
            "regenerate",
            "Validate all inputs and regenerate outputs",
            "problem_id",
        ),
        (
            "submit",
            "Judge source text or a workspace-relative attached file",
            "problem_id language ?source ?file",
        ),
        (
            "submissions_list",
            "Read submission attempts and per-test verdicts",
            "problem_id",
        ),
        (
            "test_run",
            "Judge a saved answer and compare its expected verdict",
            "problem_id answer_id",
        ),
        (
            "ai_review",
            "Independently review answer:<uuid> or test:<suite>:<number> without editing",
            "problem_id target provider",
        ),
        (
            "trust_get",
            "Read only currently applicable AI opinions",
            "problem_id target",
        ),
        (
            "reviews_list",
            "Read historical and current audit records",
            "problem_id",
        ),
        (
            "workspace_read",
            "Read UTF-8 source material inside the configured workspace",
            "path",
        ),
        (
            "capabilities",
            "List available language recipes and configured AI providers",
            "",
        ),
    ];
    definitions.into_iter().map(|(name,description,fields)|{
        let mut properties=json!({});let mut required=vec![];
        for field in fields.split_whitespace(){let optional=field.starts_with('?');let key=field.trim_start_matches('?');
            properties[key]=match key {"number"|"time_limit_ms"|"memory_limit_mb"=>json!({"type":"integer","minimum":1}),"main"=>json!({"type":"boolean"}),"args"|"tags"=>json!({"type":"array","items":{"type":"string"}}),"validator"=>json!({"type":["string","null"]}),_=>json!({"type":"string"})};
            if !optional {required.push(key);}
        }
        json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false}})
    }).collect()
}
pub fn prompt(name: &str) -> Option<&'static str> {
    match name {
        "register_problem" => Some(include_str!("../prompts/register.md")),
        "audit_problem" => Some(include_str!("../prompts/audit.md")),
        "learning_hint" => Some(include_str!("../prompts/hint.md")),
        _ => None,
    }
}
pub fn handle(engine: &Engine, request: Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let method = request["method"].as_str().unwrap_or("");
    let params = &request["params"];
    let result = match method {
        "initialize" => Ok(
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{},"prompts":{}},"serverInfo":{"name":"agentic-judge","version":env!("CARGO_PKG_VERSION")}}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools":tools()})),
        "tools/call" => {
            let result = engine.call(
                params["name"].as_str().unwrap_or(""),
                params.get("arguments").cloned().unwrap_or(json!({})),
            );
            Ok(match result {
                Ok(v) => json!({"content":[{"type":"text","text":v.to_string()}],"isError":false}),
                Err(e) => json!({"content":[{"type":"text","text":e.to_string()}],"isError":true}),
            })
        }
        "prompts/list" => Ok(
            json!({"prompts":[{"name":"register_problem","description":"Import workspace material and construct/test a problem","arguments":[{"name":"context","description":"Source paths and problem identifier","required":true}]},{"name":"audit_problem","description":"Audit code revisions and generated pairs","arguments":[{"name":"context","description":"Problem identifier","required":true}]},{"name":"learning_hint","description":"Provide progressive hints and study material","arguments":[{"name":"context","description":"Problem and learner question","required":true}]}]}),
        ),
        "prompts/get" => match prompt(params["name"].as_str().unwrap_or("")) {
            Some(p) => Ok(
                json!({"messages":[{"role":"user","content":{"type":"text","text":format!("{}\n\nContext: {}",p,params["arguments"]["context"].as_str().unwrap_or(""))}}]}),
            ),
            None => Err((-32602, "unknown prompt")),
        },
        _ => Err((-32601, "method not found")),
    };
    Some(match result {
        Ok(v) => json!({"jsonrpc":"2.0","id":id,"result":v}),
        Err((code, message)) => {
            json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
        }
    })
}
pub fn serve(engine: Engine) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(v) => handle(&engine, v),
            Err(_) => Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}),
            ),
        };
        if let Some(response) = response {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}
