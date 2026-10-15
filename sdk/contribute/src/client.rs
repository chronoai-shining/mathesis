//! A small blocking client for the Mathesis contribution API.

use serde_json::{Value, json};

pub struct Client {
    http: reqwest::blocking::Client,
    base: String,
    token: String,
    nyxid_service: Option<String>,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: u16,
    pub detail: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.detail, self.status)
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
fn segment(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

impl Client {
    /// `base` is the venue origin, e.g. `https://mathesis.example.org`;
    /// `token` a NyxID access token (or `dev:<name>` against a dev server).
    pub fn new(base: &str, token: &str) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent("mathesis-contribute/0.1")
            .build()
            .expect("HTTP client");
        Self {
            http,
            base: format!("{}/api/v1", base.trim_end_matches('/')),
            token: token.to_owned(),
            nyxid_service: None,
        }
    }

    pub fn via_nyxid(service: &str) -> Self {
        let mut client = Self::new("https://mathesis.invalid", "");
        client.nyxid_service = Some(service.to_owned());
        client
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder) -> ApiResult<Value> {
        if let Some(service) = &self.nyxid_service {
            use std::{
                io::Write,
                process::{Command, Stdio},
            };
            let request = request.build().map_err(|e| ApiError {
                status: 0,
                detail: e.to_string(),
            })?;
            let url = request.url();
            let path = match url.query() {
                Some(q) => format!("{}?{q}", url.path()),
                None => url.path().to_owned(),
            };
            let mut command = Command::new("nyxid");
            command.args([
                "proxy",
                "request",
                service,
                &path,
                "--method",
                request.method().as_str(),
            ]);
            let body = request.body().and_then(|b| b.as_bytes());
            if body.is_some() {
                command.args(["--data", "-", "--header", "Content-Type:application/json"]);
            }
            // No shell, no token on argv, and request bodies travel through stdin.
            let mut child = command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|_| ApiError {
                    status: 0,
                    detail: "Unable to start nyxid. Install the CLI and sign in first.".into(),
                })?;
            if let Some(body) = body {
                let write = child.stdin.take().unwrap().write_all(body);
                if let Err(e) = write {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ApiError {
                        status: 0,
                        detail: e.to_string(),
                    });
                }
            } else {
                drop(child.stdin.take());
            }
            let output = child.wait_with_output().map_err(|e| ApiError {
                status: 0,
                detail: e.to_string(),
            })?;
            if !output.status.success() {
                return Err(ApiError { status: 0, detail: "NyxID proxy request failed. Check your sign-in and connected Mathesis service with nyxid proxy discover.".into() });
            }
            return serde_json::from_slice(&output.stdout).map_err(|_| ApiError {
                status: 0,
                detail: "NyxID returned a non-JSON response".into(),
            });
        }

        let request = if self.token.is_empty() {
            request
        } else {
            request.bearer_auth(&self.token)
        };
        let response = request.send().map_err(|e| ApiError {
            status: 0,
            detail: e.to_string(),
        })?;
        let status = response.status().as_u16();
        let text = response.text().unwrap_or_default();
        let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            Ok(body)
        } else {
            let detail = body["detail"].as_str().map(str::to_owned).unwrap_or(text);
            Err(ApiError { status, detail })
        }
    }

    pub fn get(&self, path: &str) -> ApiResult<Value> {
        self.send(self.http.get(format!("{}{path}", self.base)))
    }

    pub fn post(&self, path: &str, body: &Value) -> ApiResult<Value> {
        self.send(self.http.post(format!("{}{path}", self.base)).json(body))
    }

    pub fn delete(&self, path: &str) -> ApiResult<Value> {
        self.send(self.http.delete(format!("{}{path}", self.base)))
    }

    pub fn formalization_opportunities(&self, args: &Value) -> ApiResult<Value> {
        let mut query = vec![];
        for key in ["q", "kind", "status", "before"] {
            if let Some(value) = args[key].as_str() {
                query.push(format!("{key}={}", segment(value)));
            }
        }
        if let Some(limit) = args["limit"].as_u64() {
            query.push(format!("limit={}", limit.clamp(1, 100)));
        }
        self.get(&format!(
            "/formalizations/opportunities?{}",
            query.join("&")
        ))
    }
    pub fn formalization_project(&self, id: &str) -> ApiResult<Value> {
        self.get(&format!("/formalizations/{}/project", segment(id)))
    }
    pub fn papers(&self, args: &Value) -> ApiResult<Value> {
        let mut query = Vec::new();
        for key in ["q", "author", "msc", "kind", "before"] {
            if let Some(value) = args[key].as_str() {
                query.push((key, value.to_owned()));
            }
        }
        query.push((
            "limit",
            args["limit"]
                .as_u64()
                .unwrap_or(25)
                .clamp(1, 100)
                .to_string(),
        ));
        self.send(self.http.get(format!("{}/papers", self.base)).query(&query))
    }
    pub fn paper(&self, record: &str) -> ApiResult<Value> {
        self.get(&format!("/papers/{}", segment(record)))
    }
    pub fn review(&self, id: &str) -> ApiResult<Value> {
        self.get(&format!("/submissions/{}/review", segment(id)))
    }
    pub fn knowledge_capabilities(&self) -> ApiResult<Value> {
        self.get("/knowledge/capabilities")
    }
    pub fn research(&self, id: &str) -> ApiResult<Value> {
        self.get(&format!("/submissions/{}/research", segment(id)))
    }
    pub fn adopt_research(&self, id: &str, body: &Value) -> ApiResult<Value> {
        self.post(&format!("/submissions/{}/research", segment(id)), body)
    }
    pub fn stop_research(&self, id: &str, key: &str, reason: &str) -> ApiResult<Value> {
        self.post(
            &format!(
                "/submissions/{}/research/{}/stop",
                segment(id),
                segment(key)
            ),
            &json!({"reason":reason}),
        )
    }
    pub fn research_attempt(&self, id: &str, key: &str, body: &Value) -> ApiResult<Value> {
        self.post(
            &format!(
                "/submissions/{}/research/{}/attempts",
                segment(id),
                segment(key)
            ),
            body,
        )
    }
    pub fn research_attempt_status(&self, id: &str, key: &str, attempt: &str) -> ApiResult<Value> {
        self.get(&format!(
            "/submissions/{}/research/{}/attempts/{}",
            segment(id),
            segment(key),
            segment(attempt)
        ))
    }
    pub fn recheck_research(&self, id: &str, key: &str, attempt: &str) -> ApiResult<Value> {
        self.post(
            &format!(
                "/submissions/{}/research/{}/attempts/{}/recheck",
                segment(id),
                segment(key),
                segment(attempt)
            ),
            &json!({}),
        )
    }
    pub fn knowledge_response(&self, id: &str, receipt: &str) -> ApiResult<Value> {
        self.get(&format!(
            "/submissions/{}/referee/knowledge/{}",
            segment(id),
            segment(receipt)
        ))
    }
    pub fn respond_review(&self, id: &str, body: &Value) -> ApiResult<Value> {
        self.post(
            &format!("/submissions/{}/review/responses", segment(id)),
            body,
        )
    }
    pub fn verification_bundle(
        &self,
        record: &str,
        claim: &str,
        attempt: &str,
    ) -> ApiResult<Value> {
        self.get(&format!(
            "/papers/{}/statements/{}/attempts/{}/bundle",
            segment(record),
            segment(claim),
            segment(attempt)
        ))
    }
    pub fn formalization(&self, id: &str) -> ApiResult<Value> {
        self.get(&format!("/formalizations/{}", segment(id)))
    }
    pub fn verify_formalization(&self, id: &str, record: &str, claim: &str) -> ApiResult<Value> {
        self.verify_formalization_against(id, record, claim, None)
    }
    pub fn verify_formalization_against(
        &self,
        id: &str,
        record: &str,
        claim: &str,
        digest: Option<&str>,
    ) -> ApiResult<Value> {
        self.verify_formalization_project(id, record, claim, digest, None)
    }
    pub fn verify_formalization_project(
        &self,
        id: &str,
        record: &str,
        claim: &str,
        digest: Option<&str>,
        solution: Option<&str>,
    ) -> ApiResult<Value> {
        self.post(
            &format!("/formalizations/{}/attempts", segment(id)),
            &json!({"record":record,"claim":claim,"expected_target":digest,"solution":solution}),
        )
    }
    pub fn conjectures(&self) -> ApiResult<Value> {
        self.get("/conjectures?limit=100")
    }
    pub fn target(&self, record: &str, claim: &str) -> ApiResult<Value> {
        self.get(&format!("/papers/{record}/statements/{claim}"))
    }
    pub fn attempt(
        &self,
        record: &str,
        claim: &str,
        solution: &str,
        agent: Option<&str>,
    ) -> ApiResult<Value> {
        self.post(
            &format!("/papers/{record}/statements/{claim}/attempts"),
            &json!({"solution": solution, "as_agent": agent}),
        )
    }
    pub fn report(&self, record: &str, body: &Value) -> ApiResult<Value> {
        self.post(&format!("/papers/{record}/errors"), body)
    }
    pub fn report_status(&self, id: &str) -> ApiResult<Value> {
        self.get(&format!("/errors/{id}"))
    }
    pub fn attempt_status(&self, id: &str) -> ApiResult<Value> {
        self.get(&format!("/attempts/{id}"))
    }

    pub fn open_tasks(&self, kind: Option<&str>, limit: u32) -> ApiResult<Value> {
        if kind == Some("formalize") {
            return Err(ApiError {
                status: 422,
                detail: "use target and attempt for statement formalization".into(),
            });
        }
        let mut path = format!("/tasks?status=open&limit={limit}");
        if let Some(kind) = kind {
            path.push_str(&format!("&kind={kind}"));
        }
        self.get(&path)
    }

    pub fn lease(&self, task: &str) -> ApiResult<Value> {
        self.post(&format!("/tasks/{task}/lease"), &json!({}))
    }

    pub fn release(&self, task: &str) -> ApiResult<Value> {
        self.delete(&format!("/tasks/{task}/lease"))
    }

    /// The task plus what an agent needs to do it: the statement, the
    /// statements it depends on, the paper's title and abstract, and the
    /// rules for its kind.
    pub fn context(&self, task: &str) -> ApiResult<Value> {
        let mut context = self.get(&format!("/tasks/{task}"))?;
        let kind = context["task"]["kind"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        context["rules"] = json!(crate::rules::for_kind(&kind));
        Ok(context)
    }

    pub fn submit(&self, task: &str, contribution: &Value) -> ApiResult<Value> {
        self.post(&format!("/tasks/{task}/contributions"), contribution)
    }
}

#[cfg(test)]
mod solving_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    #[test]
    fn solving_client_downloads_exact_target_and_submits_without_claiming_verification() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (method, path, body) in [
                (
                    "GET",
                    "/api/v1/papers?q=prime+%26+gaps&author=A.+Author&limit=25",
                    json!({"items":[],"next_before":null}),
                ),
                (
                    "GET",
                    "/api/v1/formalizations/opportunities?q=prime%20%26%20gaps&kind=paper&status=open&limit=1",
                    json!({"items":[], "next_before": "cursor"}),
                ),
                (
                    "GET",
                    "/api/v1/submissions/sub1/review",
                    json!({"schema_version":1,"source_sha256":"pinned-source","service":"Orchestra"}),
                ),
                (
                    "GET",
                    "/api/v1/conjectures?limit=100",
                    json!({"items": [], "next_before": null}),
                ),
                (
                    "GET",
                    "/api/v1/papers/WP-2026-0001/statements/C1",
                    json!({"target": {"lean":"import Mathlib\ndef wishpool_target_prop : Prop := True\n", "digest":"exact-digest"}}),
                ),
                (
                    "POST",
                    "/api/v1/papers/WP-2026-0001/statements/C1/attempts",
                    json!({"id":"a1", "state":"queued"}),
                ),
                (
                    "GET",
                    "/api/v1/attempts/a1",
                    json!({"id":"a1", "state":"rejected", "receipt":{"reason":"Wrong target type"}}),
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let header_end = loop {
                    let mut buf = [0; 4096];
                    let count = stream.read(&mut buf).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buf[..count]);
                    if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break pos + 4;
                    }
                };
                let header = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                assert!(header.starts_with(&format!("{method} {path} HTTP/1.1")));
                assert!(
                    header
                        .to_lowercase()
                        .contains("authorization: bearer test-token")
                );
                let length = header
                    .lines()
                    .find_map(|l| {
                        l.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|n| n.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let mut buf = [0; 4096];
                    let count = stream.read(&mut buf).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buf[..count]);
                }
                if method == "POST" {
                    let submitted: Value = serde_json::from_slice(&bytes[header_end..]).unwrap();
                    assert_eq!(
                        submitted,
                        json!({"solution":"import Target\nproof", "as_agent":"Proof agent"})
                    );
                    assert!(submitted.get("receipt").is_none());
                }
                let text = body.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).unwrap();
            }
        });
        let client = Client::new(&origin, "test-token");
        assert_eq!(
            client
                .papers(&json!({"q":"prime & gaps","author":"A. Author"}))
                .unwrap()["items"],
            json!([])
        );
        assert_eq!(
            client
                .formalization_opportunities(
                    &json!({"q":"prime & gaps", "kind":"paper", "status":"open", "limit":1})
                )
                .unwrap()["next_before"],
            "cursor"
        );
        assert_eq!(
            client.review("sub1").unwrap()["source_sha256"],
            "pinned-source"
        );
        assert_eq!(client.conjectures().unwrap()["items"], json!([]));
        let target = client.target("WP-2026-0001", "C1").unwrap();
        assert_eq!(target["target"]["digest"], "exact-digest");
        assert!(
            target["target"]["lean"]
                .as_str()
                .unwrap()
                .ends_with("True\n")
        );
        assert_eq!(
            client
                .attempt(
                    "WP-2026-0001",
                    "C1",
                    "import Target\nproof",
                    Some("Proof agent")
                )
                .unwrap()["state"],
            "queued"
        );
        assert_eq!(
            client.attempt_status("a1").unwrap()["receipt"]["reason"],
            "Wrong target type"
        );
        server.join().unwrap();
    }
}
