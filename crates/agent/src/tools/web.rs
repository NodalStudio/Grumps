//! Web search tool with provider abstraction.
//! Spec § 10. Default provider = Brave Base ($3/CPM), alternative = SearXNG (OSS, self-host).

use worker::*;
use serde::{Deserialize, Serialize};
use crate::tools::ToolContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hit {
    pub title: String,
    pub url: String,
    pub description: String,
    #[serde(default)]
    pub published: Option<String>,
}

#[async_trait::async_trait(?Send)]
pub trait WebSearchProvider {
    async fn search(&self, query: &str, count: u8, freshness: &str) -> Result<Vec<Hit>>;
}

pub struct BraveProvider { pub api_key: String }
pub struct SearxngProvider { pub instance_url: String, pub api_key: Option<String> }
pub struct StubProvider;

#[async_trait::async_trait(?Send)]
impl WebSearchProvider for BraveProvider {
    async fn search(&self, query: &str, count: u8, freshness: &str) -> Result<Vec<Hit>> {
        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}&freshness={}",
            urlencode(query), count.min(10), if freshness == "all" { "" } else { freshness }
        );
        let mut headers = Headers::new();
        headers.set("X-Subscription-Token", &self.api_key)?;
        headers.set("Accept", "application/json")?;
        let req = Request::new_with_init(&url, RequestInit::new()
            .with_method(Method::Get)
            .with_headers(headers))?;
        let mut resp = Fetch::Request(req).send().await?;
        if resp.status_code() >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::RustError(format!("Brave {}: {body}", resp.status_code())));
        }
        #[derive(Deserialize)]
        struct BraveResp {
            #[serde(default)]
            web: Option<BraveWeb>,
        }
        #[derive(Deserialize)]
        struct BraveWeb {
            #[serde(default)]
            results: Vec<BraveHit>,
        }
        #[derive(Deserialize)]
        struct BraveHit {
            title: String, url: String,
            #[serde(default)] description: String,
            #[serde(default)] age: Option<String>,
        }
        let parsed: BraveResp = resp.json().await
            .map_err(|e| Error::RustError(format!("Brave parse: {e}")))?;
        Ok(parsed.web.map(|w| w.results).unwrap_or_default().into_iter().map(|h| Hit {
            title: h.title, url: h.url, description: h.description, published: h.age,
        }).collect())
    }
}

#[async_trait::async_trait(?Send)]
impl WebSearchProvider for SearxngProvider {
    async fn search(&self, query: &str, count: u8, _freshness: &str) -> Result<Vec<Hit>> {
        let url = format!("{}/search?q={}&format=json&safesearch=0",
            self.instance_url.trim_end_matches('/'), urlencode(query));
        let mut headers = Headers::new();
        headers.set("Accept", "application/json")?;
        if let Some(key) = &self.api_key {
            headers.set("Authorization", &format!("Bearer {key}"))?;
        }
        let req = Request::new_with_init(&url, RequestInit::new()
            .with_method(Method::Get)
            .with_headers(headers))?;
        let mut resp = Fetch::Request(req).send().await?;
        if resp.status_code() >= 400 {
            return Err(Error::RustError(format!("SearXNG {}", resp.status_code())));
        }
        #[derive(Deserialize)]
        struct SearxResp { results: Vec<SearxHit> }
        #[allow(non_snake_case)]
        #[derive(Deserialize)]
        struct SearxHit {
            title: String, url: String,
            #[serde(default)] content: String,
            #[serde(default)] publishedDate: Option<String>,
        }
        let parsed: SearxResp = resp.json().await
            .map_err(|e| Error::RustError(format!("SearXNG parse: {e}")))?;
        Ok(parsed.results.into_iter().take(count as usize).map(|h| Hit {
            title: h.title, url: h.url, description: h.content, published: h.publishedDate,
        }).collect())
    }
}

#[async_trait::async_trait(?Send)]
impl WebSearchProvider for StubProvider {
    async fn search(&self, _q: &str, _c: u8, _f: &str) -> Result<Vec<Hit>> {
        Ok(vec![])
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

pub fn build_provider(env: &Env) -> Box<dyn WebSearchProvider> {
    let provider = env.var("WEB_SEARCH_PROVIDER").map(|v| v.to_string()).unwrap_or_else(|_| "brave".into());
    match provider.as_str() {
        "searxng" => {
            let url = env.var("SEARXNG_URL").map(|v| v.to_string()).unwrap_or_default();
            let key = env.secret("SEARXNG_API_KEY").ok().map(|s| s.to_string());
            Box::new(SearxngProvider { instance_url: url, api_key: key })
        }
        "brave" => {
            match env.secret("BRAVE_API_KEY") {
                Ok(k) => Box::new(BraveProvider { api_key: k.to_string() }),
                Err(_) => {
                    worker::console_log!("BRAVE_API_KEY missing, using stub");
                    Box::new(StubProvider)
                }
            }
        }
        _ => Box::new(StubProvider),
    }
}

/// Tool entry point.
pub async fn web_search(ctx: &ToolContext<'_>, args: serde_json::Value) -> Result<serde_json::Value> {
    let query = args.get("query").and_then(|v| v.as_str())
        .ok_or_else(|| Error::RustError("web_search: missing 'query'".into()))?;
    let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(5).min(10) as u8;
    let freshness = args.get("freshness").and_then(|v| v.as_str()).unwrap_or("all");

    // KV cache : 1h TTL on (query, freshness)
    let kv = ctx.env.kv("KV").ok();
    let cache_key = format!("ws:{}:{}", simple_hash(query), freshness);
    if let Some(ref kv) = kv {
        if let Ok(Some(cached)) = kv.get(&cache_key).text().await {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&cached) {
                return Ok(parsed);
            }
        }
    }

    // Quota check via settings
    let used = ctx.db.get_int_setting("web_search_quota_used_month").await.unwrap_or(0);
    let plan = ctx.db.get_setting("plan").await.unwrap_or_else(|_| "free".into());
    let limit = match plan.as_str() {
        "pro" => 50,
        "business" => 500,
        _ => 5,
    };
    if used >= limit {
        return Ok(serde_json::json!({
            "error": "quota_exceeded",
            "message": format!("Tu as utilisé tes {limit} recherches web ce mois-ci. Plan {plan}."),
            "used": used,
            "limit": limit,
        }));
    }

    // Call provider
    let provider = build_provider(ctx.env);
    let hits = provider.search(query, count, freshness).await?;
    let result = serde_json::json!({ "query": query, "results": hits });

    // Cache + bump quota
    if let Some(ref kv) = kv {
        let _ = kv.put(&cache_key, &result.to_string()).map(|p| p.expiration_ttl(3600).execute());
    }
    ctx.db.increment_int_setting("web_search_quota_used_month", 1).await.ok();

    Ok(result)
}

fn simple_hash(s: &str) -> String {
    // Cheap deterministic hash for KV keys (not crypto)
    let mut h: u64 = 14695981039346656037;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    format!("{:x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urlencode_handles_special() {
        assert_eq!(urlencode("a b"), "a%20b");
        assert_eq!(urlencode("hello"), "hello");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
    }
    #[test]
    fn simple_hash_deterministic() {
        let a = simple_hash("test");
        let b = simple_hash("test");
        assert_eq!(a, b);
        let c = simple_hash("other");
        assert_ne!(a, c);
    }
}
