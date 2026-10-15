//! Bounded read-only observations of the existing public GitHub API.
//! Network transport and Git ancestry are facts, never mathematical admission.
use super::{PublicationIdentity, sha256};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const REPOSITORY: &str = "the-omega-institute/trureturing";
const MAX_RESPONSE: usize = 350_000;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitObservation {
    pub repository: String,
    pub base: String,
    pub state: String,
    pub merge_commit: Option<String>,
    pub merged_at: Option<String>,
    pub included: bool,
    pub limits: Vec<String>,
    pub responses: Vec<FetchEvidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchEvidence {
    pub url: String,
    pub status: Option<u16>,
    pub body: String,
    pub body_sha256: String,
    pub error: Option<String>,
}
fn git_id(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
async fn fetch(client: &reqwest::Client, path: &str) -> FetchEvidence {
    fetch_url(
        client,
        format!("https://api.github.com/repos/{REPOSITORY}/{path}"),
    )
    .await
}
async fn fetch_url(client: &reqwest::Client, url: String) -> FetchEvidence {
    let mut evidence = FetchEvidence {
        url: url.clone(),
        status: None,
        body: String::new(),
        body_sha256: sha256(&[]),
        error: None,
    };
    match client.get(url).send().await {
        Err(_) => evidence.error = Some("public GitHub request failed or timed out".into()),
        Ok(mut response) => {
            evidence.status = Some(response.status().as_u16());
            let mut body = vec![];
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) if body.len() + chunk.len() <= MAX_RESPONSE => {
                        body.extend_from_slice(&chunk)
                    }
                    Ok(None) => break,
                    Ok(Some(_)) => {
                        evidence.error =
                            Some("GitHub response exceeds 350000-byte observation limit".into());
                        break;
                    }
                    Err(_) => {
                        evidence.error = Some("GitHub response interrupted".into());
                        break;
                    }
                }
            }
            evidence.body_sha256 = sha256(&body);
            match String::from_utf8(body) {
                Ok(text) => evidence.body = text,
                Err(_) => evidence.error = Some("GitHub response is not UTF-8".into()),
            }
            if evidence.status != Some(200) && evidence.error.is_none() {
                evidence.error = Some(
                    "GitHub response unavailable; public unauthenticated API rate limits apply"
                        .into(),
                );
            }
        }
    }
    evidence
}
fn parsed(e: &FetchEvidence) -> Option<Value> {
    if e.status == Some(200) && e.error.is_none() {
        serde_json::from_str(&e.body).ok()
    } else {
        None
    }
}
fn pr_metadata(number: u64, v: &Value) -> Option<(String, String, Option<String>, Option<String>)> {
    if v["number"].as_u64() != Some(number) || v["base"]["repo"]["full_name"] != REPOSITORY {
        return None;
    }
    let base = v["base"]["ref"].as_str()?.to_owned();
    let merged = v["merged"] == true && v["state"] == "closed";
    let merge = v["merge_commit_sha"]
        .as_str()
        .filter(|s| git_id(s))
        .map(str::to_owned);
    let at = v["merged_at"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let state = if merged && merge.is_some() && at.is_some() {
        "merged"
    } else if v["state"] == "open" {
        "open"
    } else {
        "closed_unmerged"
    };
    Some((
        base,
        state.into(),
        merged.then_some(merge).flatten(),
        merged.then_some(at).flatten(),
    ))
}
fn included(
    publication: &PublicationIdentity,
    merge: &str,
    commit: &Value,
    compare: &Value,
) -> bool {
    commit["sha"] == publication.source_commit
        && commit["tree"]["sha"] == publication.source_tree
        && compare["base_commit"]["sha"] == merge
        && compare["merge_base_commit"]["sha"] == merge
        && matches!(compare["status"].as_str(), Some("ahead" | "identical"))
}
pub async fn observe(number: u64, publication: &PublicationIdentity) -> GitObservation {
    let mut out = GitObservation {
        repository: REPOSITORY.into(),
        base: String::new(),
        state: "unavailable".into(),
        merge_commit: None,
        merged_at: None,
        included: false,
        limits: vec!["Read-only public GitHub observation; merge and ancestry do not establish mathematical correspondence or freeze.".into()],
        responses: vec![],
    };
    if number == 0 || publication.validate().is_err() {
        out.limits.push("invalid observation identity".into());
        return out;
    }
    let client = match reqwest::Client::builder()
        .user_agent("Mathesis-publication-readback/1")
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            out.limits.push("GitHub client unavailable".into());
            return out;
        }
    };
    let pr = fetch(&client, &format!("pulls/{number}")).await;
    let metadata = parsed(&pr).and_then(|v| pr_metadata(number, &v));
    out.responses.push(pr);
    if let Some((base, state, merge, at)) = metadata {
        out.base = base;
        out.state = state;
        out.merge_commit = merge;
        out.merged_at = at;
    } else {
        out.limits
            .push("PR identity or response could not be verified".into());
    }
    if out.base == "dev"
        && out.state == "merged"
        && let Some(merge) = out.merge_commit.as_deref()
    {
        let commit = fetch(
            &client,
            &format!("git/commits/{}", publication.source_commit),
        )
        .await;
        let compare = fetch(
            &client,
            &format!("compare/{merge}...{}?per_page=1", publication.source_commit),
        )
        .await;
        out.included = parsed(&commit)
            .zip(parsed(&compare))
            .is_some_and(|(c, d)| included(publication, merge, &c, &d));
        out.responses.extend([commit, compare]);
        if !out.included {
            out.limits
                .push("PR merge inclusion in the admitted source was not established".into());
        }
    }
    for r in &out.responses {
        if let Some(e) = &r.error {
            out.limits.push(e.clone());
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn merge_metadata_and_exact_ancestry_are_separate_checks() {
        let merge = "a".repeat(40);
        let source = "b".repeat(40);
        let tree = "c".repeat(40);
        let p = PublicationIdentity {
            bundle_digest: format!("sha256:{}", "d".repeat(64)),
            source_commit: source.clone(),
            source_tree: tree.clone(),
        };
        let mut pr = json!({"number":9,"base":{"repo":{"full_name":REPOSITORY},"ref":"dev"},"merged":true,"state":"closed","merge_commit_sha":merge,"merged_at":"2026-10-16T00:00:00Z"});
        assert_eq!(pr_metadata(9, &pr).unwrap().1, "merged");
        pr["merged"] = json!(false);
        assert_eq!(pr_metadata(9, &pr).unwrap().1, "closed_unmerged");
        assert!(pr_metadata(10, &pr).is_none());
        pr["base"]["repo"]["full_name"] = json!("attacker/trureturing");
        assert!(pr_metadata(9, &pr).is_none());
        let commit = json!({"sha":source,"tree":{"sha":tree}});
        let mut compare =
            json!({"base_commit":{"sha":merge},"merge_base_commit":{"sha":merge},"status":"ahead"});
        assert!(included(&p, &merge, &commit, &compare));
        compare["merge_base_commit"]["sha"] = json!("e".repeat(40));
        assert!(!included(&p, &merge, &commit, &compare));
        compare["merge_base_commit"]["sha"] = json!(merge);
        compare["status"] = json!("diverged");
        assert!(!included(&p, &merge, &commit, &compare));
        let mut changed = commit;
        changed["tree"]["sha"] = json!("f".repeat(40));
        compare["status"] = json!("ahead");
        assert!(!included(&p, &merge, &changed, &compare));
    }
    #[tokio::test]
    async fn transport_preserves_bytes_and_refuses_redirects_large_bodies_and_failed_statuses() {
        use axum::{
            Router,
            http::StatusCode,
            response::{IntoResponse, Redirect},
            routing::get,
        };
        let server = Router::new()
            .route("/ok", get(|| async { "{\"fact\":true}" }))
            .route("/large", get(|| async { "x".repeat(MAX_RESPONSE + 1) }))
            .route("/redirect", get(|| async { Redirect::temporary("/ok") }))
            .route(
                "/failed",
                get(|| async { (StatusCode::TOO_MANY_REQUESTS, "rate limited").into_response() }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        });
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap();
        let ok = fetch_url(&client, format!("http://{addr}/ok")).await;
        assert_eq!(ok.body_sha256, sha256(ok.body.as_bytes()));
        assert_eq!(parsed(&ok).unwrap()["fact"], true);
        for path in ["large", "redirect", "failed"] {
            let e = fetch_url(&client, format!("http://{addr}/{path}")).await;
            assert!(e.error.is_some());
            assert!(parsed(&e).is_none());
            assert!(e.body.len() <= MAX_RESPONSE);
            assert_eq!(e.body_sha256, sha256(e.body.as_bytes()));
        }
        task.abort();
    }
}
