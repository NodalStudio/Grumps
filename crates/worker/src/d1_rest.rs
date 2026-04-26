use serde::{Deserialize, Serialize};
use worker::*;

pub struct D1RestClient {
    account_id: String,
    api_token: String,
}

#[derive(Debug, Serialize)]
struct D1Query {
    sql: String,
    params: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct D1Response {
    pub result: Vec<D1ResultSet>,
    pub success: bool,
    pub errors: Vec<D1Error>,
}

#[derive(Debug, Deserialize)]
pub struct D1ResultSet {
    pub results: Option<Vec<serde_json::Value>>,
    pub meta: Option<D1Meta>,
}

#[derive(Debug, Deserialize)]
pub struct D1Meta {
    pub changes: Option<i64>,
    pub last_row_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct D1Error {
    pub message: String,
}

impl D1RestClient {
    pub fn from_env(env: &Env) -> Result<Self> {
        Ok(Self {
            account_id: env.secret("CF_ACCOUNT_ID")?.to_string(),
            api_token: env.secret("CF_API_TOKEN")?.to_string(),
        })
    }

    pub async fn query(&self, database_id: &str, sql: &str, params: Vec<serde_json::Value>) -> Result<D1Response> {
        let url = format!("https://api.cloudflare.com/client/v4/accounts/{}/d1/database/{}/query", self.account_id, database_id);
        let body = serde_json::to_string(&D1Query { sql: sql.to_string(), params }).map_err(|e| Error::RustError(e.to_string()))?;

        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", self.api_token))?;
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.into()));

        let req = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;
        let response: D1Response = resp.json().await?;

        if !response.success {
            let msg = response.errors.first().map(|e| e.message.clone()).unwrap_or("unknown D1 error".into());
            return Err(Error::RustError(msg));
        }
        Ok(response)
    }

    pub async fn exec_statements(&self, database_id: &str, sql: &str) -> Result<()> {
        // Normalize CRLF to LF (SQLite on D1 rejects \r\n inside CREATE TRIGGER
        // bodies with "incomplete input"; Windows-saved migration files carry
        // CRLF through include_str!).
        //
        // Send the whole migration as a single /query call: D1 accepts
        // semicolon-separated multi-statement SQL, and this keeps us well
        // below the Worker subrequest limit (which an N-statement loop would
        // explode past).
        let sql = sql.replace("\r\n", "\n");
        self.query(database_id, &sql, vec![]).await?;
        Ok(())
    }

    pub async fn create_database(&self, name: &str) -> Result<String> {
        let url = format!("https://api.cloudflare.com/client/v4/accounts/{}/d1/database", self.account_id);
        let body = serde_json::json!({"name": name});

        let headers = Headers::new();
        headers.set("Authorization", &format!("Bearer {}", self.api_token))?;
        headers.set("Content-Type", "application/json")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post).with_headers(headers).with_body(Some(body.to_string().into()));

        let req = Request::new_with_init(&url, &init)?;
        let mut resp = Fetch::Request(req).send().await?;

        #[derive(Deserialize)]
        struct Resp { result: Option<DbResult>, success: bool }
        #[derive(Deserialize)]
        struct DbResult { uuid: String }

        let result: Resp = resp.json().await?;
        if !result.success { return Err(Error::RustError("Failed to create D1 database".into())); }
        Ok(result.result.unwrap().uuid)
    }
}

pub fn extract_rows<T: serde::de::DeserializeOwned>(response: &D1Response) -> Result<Vec<T>> {
    let results = response.result.first().and_then(|r| r.results.as_ref()).cloned().unwrap_or_default();
    results.into_iter().map(|v| serde_json::from_value(v).map_err(|e| Error::RustError(e.to_string()))).collect()
}

pub fn extract_first<T: serde::de::DeserializeOwned>(response: &D1Response) -> Result<Option<T>> {
    let rows: Vec<T> = extract_rows(response)?;
    Ok(rows.into_iter().next())
}

/// Split multi-statement SQL on top-level `;` while keeping trigger bodies
/// (`BEGIN ... END;`) intact. D1's `/query` endpoint accepts only one
/// statement per call, so we must split client-side.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth: usize = 0;
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if starts_with_word_ci(bytes, i, b"BEGIN") {
            depth += 1;
            current.push_str("BEGIN");
            i += 5;
            continue;
        }
        if depth > 0 && starts_with_word_ci(bytes, i, b"END") {
            depth -= 1;
            current.push_str("END");
            i += 3;
            continue;
        }
        let c = bytes[i];
        if c == b';' && depth == 0 {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            current.clear();
            i += 1;
            continue;
        }
        current.push(c as char);
        i += 1;
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        out.push(trimmed);
    }
    out
}

fn starts_with_word_ci(bytes: &[u8], i: usize, word: &[u8]) -> bool {
    if i > 0 {
        let prev = bytes[i - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return false;
        }
    }
    if i + word.len() > bytes.len() {
        return false;
    }
    for (j, wb) in word.iter().enumerate() {
        if bytes[i + j].to_ascii_uppercase() != *wb {
            return false;
        }
    }
    let next_i = i + word.len();
    if next_i < bytes.len() {
        let next = bytes[next_i];
        if next.is_ascii_alphanumeric() || next == b'_' {
            return false;
        }
    }
    true
}

