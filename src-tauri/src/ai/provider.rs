use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::db::models::WorkspaceSettings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub json_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub lmstudio_available: bool,
    pub openai_configured: bool,
    pub active_provider: String,
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<String, String>;
    fn name(&self) -> &str;
}

pub struct LMStudioProvider {
    pub base_url: String,
    pub model: String,
}

pub struct OpenAIProvider {
    pub api_key: String,
    pub model: String,
}

fn openai_payload(model: &str, req: &CompletionRequest) -> serde_json::Value {
    let msgs = vec![
        serde_json::json!({"role": "system", "content": req.system}),
        serde_json::json!({"role": "user", "content": req.user}),
    ];
    let mut body = serde_json::json!({
        "model": model,
        "messages": msgs,
        "temperature": req.temperature,
        "max_tokens": req.max_tokens,
    });
    if req.json_mode {
        body["response_format"] = serde_json::json!({"type": "json_object"});
    }
    body
}

fn shared_client(timeout_secs: u64) -> reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT_60: OnceLock<reqwest::Client> = OnceLock::new();
    static CLIENT_3: OnceLock<reqwest::Client> = OnceLock::new();
    static CLIENT_2: OnceLock<reqwest::Client> = OnceLock::new();
    let (slot, secs) = match timeout_secs {
        60 => (&CLIENT_60, 60),
        3 => (&CLIENT_3, 3),
        _ => (&CLIENT_2, 2),
    };
    slot.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(secs))
            .build()
            .expect("reqwest Client build")
    })
    .clone()
}

#[async_trait]
impl LLMProvider for LMStudioProvider {
    fn name(&self) -> &str {
        "lmstudio"
    }
    async fn complete(&self, req: CompletionRequest) -> Result<String, String> {
        let model = if self.model == "auto" || self.model.is_empty() {
            // discover via /v1/models if auto
            match try_discover_lmstudio_model(&self.base_url).await {
                Some(m) => m,
                None => "local-model".to_string(),
            }
        } else {
            self.model.clone()
        };
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let client = shared_client(60);
        let body = openai_payload(&model, &req);
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("LM Studio requête échouée: {e}"))?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("LM Studio erreur: {txt}"));
        }
        let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        extract_content(&v)
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }
    async fn complete(&self, req: CompletionRequest) -> Result<String, String> {
        let client = shared_client(60);
        let body = openai_payload(&self.model, &req);
        let resp = client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenAI requête échouée: {e}"))?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI erreur: {txt}"));
        }
        let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        extract_content(&v)
    }
}

fn extract_content(v: &serde_json::Value) -> Result<String, String> {
    v.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "réponse LLM sans content".to_string())
}

async fn try_discover_lmstudio_model(base_url: &str) -> Option<String> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = shared_client(3);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    v.get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|m| m.get("id").and_then(|x| x.as_str()))
        .map(|s| s.to_string())
}

pub async fn lmstudio_available(base_url: &str) -> bool {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let client = shared_client(2);
    match client.get(&url).send().await {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}

fn is_openai_configured(settings: &WorkspaceSettings) -> bool {
    !settings.openai_api_key.trim().is_empty()
}

fn active_name(lm_available: bool, settings: &WorkspaceSettings) -> String {
    // Normalisation identique à get_provider ("Hybrid" doit matcher).
    let mode = settings.llm_provider.to_lowercase();
    if lm_available && (mode == "hybrid" || mode == "lmstudio") {
        "lmstudio".to_string()
    } else if is_openai_configured(settings) && (mode == "hybrid" || mode == "openai") {
        "openai".to_string()
    } else {
        "heuristic".to_string()
    }
}

/// Factory hybride : LM Studio si dispo, sinon OpenAI si clé présente, sinon None.
/// `WorkspaceSettings.llm_provider` peut forcer "lmstudio"/"openai"/"hybrid"/"off".
pub async fn get_provider(settings: &WorkspaceSettings) -> Option<Box<dyn LLMProvider>> {
    let mode = settings.llm_provider.to_lowercase();
    if mode == "off" {
        return None;
    }
    if mode == "hybrid" {
        if lmstudio_available(&settings.lmstudio_url).await {
            return Some(Box::new(LMStudioProvider {
                base_url: settings.lmstudio_url.clone(),
                model: settings.lmstudio_model.clone(),
            }));
        }
        if is_openai_configured(settings) {
            return Some(Box::new(OpenAIProvider {
                api_key: settings.openai_api_key.clone(),
                model: settings.openai_model.clone(),
            }));
        }
        return None;
    }
    if mode == "openai" {
        if is_openai_configured(settings) {
            return Some(Box::new(OpenAIProvider {
                api_key: settings.openai_api_key.clone(),
                model: settings.openai_model.clone(),
            }));
        }
        return None;
    }
    if mode == "lmstudio" {
        return Some(Box::new(LMStudioProvider {
            base_url: settings.lmstudio_url.clone(),
            model: settings.lmstudio_model.clone(),
        }));
    }
    None
}

pub async fn check_status(settings: &WorkspaceSettings) -> ProviderStatus {
    let lm = lmstudio_available(&settings.lmstudio_url).await;
    ProviderStatus {
        lmstudio_available: lm,
        openai_configured: is_openai_configured(settings),
        active_provider: active_name(lm, settings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_json_mode() {
        let req = CompletionRequest {
            system: "s".into(),
            user: "u".into(),
            max_tokens: 100,
            temperature: 0.2,
            json_mode: true,
        };
        let p = openai_payload("gpt-4o-mini", &req);
        assert_eq!(p["response_format"]["type"], "json_object");
    }
}
