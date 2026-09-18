use crate::{engine::Engine, model::*, storage::write_json};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, time::Duration};

impl Engine {
    pub fn review(&self, id: &str, target: &str, provider_id: &str) -> Result<Review> {
        let provider = self
            .runtime
            .config
            .providers
            .get(provider_id)
            .context("unknown provider")?;
        let fingerprint = self.fingerprint(id, target)?;
        let mut review = Review {
            id: uuid(),
            provider: provider_id.into(),
            requested_model: Some(provider.model.clone()),
            response_model: None,
            target: target.into(),
            fingerprint,
            status: "error".into(),
            rationale: String::new(),
            suggested_direction: String::new(),
            created_at: now(),
        };
        match self.request_review(id, target, provider, &mut review.response_model) {
            Ok(v) => {
                review.status = v["status"].as_str().context("missing status")?.into();
                review.rationale = v["rationale"].as_str().context("missing rationale")?.into();
                review.suggested_direction = v["suggested_direction"].as_str().unwrap_or("").into();
            }
            Err(e) => {
                review.rationale = e.to_string();
            }
        }
        write_json(
            &self
                .store
                .problem(id)?
                .join(format!("reviews/{}.json", review.id)),
            &review,
        )?;
        Ok(review)
    }
    fn request_review(
        &self,
        id: &str,
        target: &str,
        p: &Provider,
        response_model: &mut Option<String>,
    ) -> Result<Value> {
        let m = self.store.manifest(id)?;
        let mut context = json!({"manifest":m});
        for s in ["legend", "input", "output", "notes"] {
            context[s] = json!(fs::read_to_string(
                self.store.problem(id)?.join(format!("statements/{s}.md"))
            )?);
        }
        if let Some(a) = target.strip_prefix("answer:") {
            context["answer"] = serde_json::to_value(self.store.answer(id, a)?)?;
        } else {
            let parts: Vec<_> = target.split(':').collect();
            ensure!(parts.len() == 3, "invalid test target");
            let n = parts[2].parse()?;
            context["input"] = json!(fs::read_to_string(
                self.store.test_path(id, parts[1], n, "in")?
            )?);
            context["output"] = json!(fs::read_to_string(
                self.store.test_path(id, parts[1], n, "out")?
            )?);
        }
        let system = include_str!("../prompts/review.md");
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut req = client.post(&p.endpoint);
        if let Some(env) = &p.api_key_env {
            let key = std::env::var(env).context("provider key environment variable missing")?;
            req = if p.kind == "anthropic" {
                req.header("x-api-key", key)
                    .header("anthropic-version", "2023-06-01")
            } else {
                req.bearer_auth(key)
            };
        }
        let body = match p.kind.as_str() {
            "openai-compatible" => {
                json!({"model":p.model,"messages":[{"role":"system","content":system},{"role":"user","content":context.to_string()}],"response_format":{"type":"json_object"}})
            }
            "anthropic" => {
                json!({"model":p.model,"max_tokens":4096,"system":system,"messages":[{"role":"user","content":context.to_string()}]})
            }
            "ollama" => {
                json!({"model":p.model,"stream":false,"format":"json","messages":[{"role":"system","content":system},{"role":"user","content":context.to_string()}]})
            }
            _ => anyhow::bail!("unsupported provider kind"),
        };
        let response = req.json(&body).send()?.error_for_status()?;
        let value: Value = response.json()?;
        *response_model = value["model"]
            .as_str()
            .filter(|m| !m.trim().is_empty())
            .map(str::to_owned);
        let text = match p.kind.as_str() {
            "anthropic" => value["content"][0]["text"].as_str(),
            "ollama" => value["message"]["content"].as_str(),
            _ => value["choices"][0]["message"]["content"].as_str(),
        }
        .context("provider did not return text")?;
        let value: Value = serde_json::from_str(text)?;
        ensure!(
            ["valid", "invalid", "uncertain"].contains(&value["status"].as_str().unwrap_or("")),
            "invalid review status"
        );
        ensure!(
            value["rationale"].is_string() && value["suggested_direction"].is_string(),
            "malformed review"
        );
        Ok(value)
    }
}
