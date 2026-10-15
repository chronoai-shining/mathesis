use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::{
    config::{AdvisorConfig, Config, OracleConfig},
    store::MongoStore,
};

pub struct RuntimeReadiness {
    config: Config,
    cma: Option<Arc<mathesis_review::cma::Client>>,
    cached: Mutex<Option<(Instant, bool, Value)>>,
}

impl RuntimeReadiness {
    pub fn new(config: Config, cma: Option<Arc<mathesis_review::cma::Client>>) -> Self {
        Self {
            config,
            cma,
            cached: Mutex::new(None),
        }
    }

    pub async fn check(&self) -> (bool, Value) {
        if !self.config.role.runs_workers() {
            return (
                true,
                json!({"execution": "worker_only", "provider_execution_verified": false}),
            );
        }
        let mut cached = self.cached.lock().await;
        if let Some((at, ready, value)) = cached.as_ref()
            && at.elapsed() < Duration::from_secs(30)
        {
            return (*ready, value.clone());
        }
        let mut ready = true;
        let compile = if self.config.role.runs_workers() {
            if executable(PathBuf::from(&self.config.tex_bin).join("pdflatex")) {
                "ready"
            } else {
                ready = false;
                "unavailable"
            }
        } else {
            "worker_only"
        };
        let cma = if let Some(client) = &self.cma {
            if matches!(
                tokio::time::timeout(Duration::from_secs(8), client.preflight()).await,
                Ok(Ok(()))
            ) {
                "resources_ready"
            } else {
                ready = false;
                "unavailable"
            }
        } else {
            "disabled"
        };
        let verifier = if let Some(url) = &self.config.verifier_url {
            let available = match reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(5))
                .build()
            {
                Ok(client) => client
                    .get(format!("{}/readyz", url.trim_end_matches('/')))
                    .send()
                    .await
                    .is_ok_and(|response| response.status().is_success()),
                Err(_) => false,
            };
            if available {
                "ready"
            } else {
                ready = false;
                "unavailable"
            }
        } else if let Some(workspace) = &self.config.lean_workspace {
            let workspace = PathBuf::from(workspace);
            if executable(PathBuf::from(&self.config.verifier_program))
                && mathesis_verifier::workspace_ready(&workspace).is_ok()
            {
                "ready"
            } else {
                ready = false;
                "unavailable"
            }
        } else {
            "disabled"
        };
        let audit = match &self.config.advisor {
            Some(AdvisorConfig::Cma(_)) => cma == "resources_ready",
            Some(AdvisorConfig::Codex { program, .. }) => executable(PathBuf::from(program)),
            _ => false,
        };
        let review_enabled = self.config.oracle.is_some();
        let oracle = match &self.config.oracle {
            Some(OracleConfig::Cli { program, .. }) if !executable(PathBuf::from(program)) => {
                ready = false;
                "unavailable"
            }
            Some(_) => "configured_execution_unverified",
            None => "disabled",
        };
        if review_enabled && !audit {
            ready = false;
        }
        let value = json!({
            "compile": compile,
            "oracle": oracle,
            "cma": cma,
            "auditor_configured": audit,
            "verifier": verifier,
            "automatic_review": review_enabled && audit && compile == "ready" && oracle != "unavailable",
            "provider_execution_verified": false
        });
        *cached = Some((Instant::now(), ready, value.clone()));
        (ready, value)
    }
}

fn executable(path: PathBuf) -> bool {
    let candidates = if path.components().count() > 1 {
        vec![path]
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|directory| directory.join(&path))
            .collect()
    };
    candidates.into_iter().any(|candidate| {
        let Ok(metadata) = candidate.metadata() else {
            return false;
        };
        if !metadata.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}

#[derive(Clone)]
pub struct Readiness {
    pub mongo: Option<MongoStore>,
    pub runtime: Option<Arc<RuntimeReadiness>>,
}

impl Readiness {
    pub async fn check(&self) -> (bool, Value) {
        let database_ready = match &self.mongo {
            Some(store) => store.ping().await.is_ok(),
            None => true,
        };
        let (services_ready, services) = match &self.runtime {
            Some(runtime) => runtime.check().await,
            None => (true, json!({})),
        };
        let ready = database_ready && services_ready;
        (
            ready,
            json!({
                "status": if ready { "ready" } else { "unavailable" },
                "storage": if self.mongo.is_some() { "mongo" } else { "memory" },
                "database_ready": database_ready,
                "services": services
            }),
        )
    }
}

pub fn routes(readiness: Arc<Readiness>) -> Router {
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(json!({ "status": "ok" })) }),
        )
        .route("/readyz", get(ready))
        .with_state(readiness)
}

async fn ready(State(readiness): State<Arc<Readiness>>) -> impl IntoResponse {
    let (ready, value) = readiness.check().await;
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(value),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::from_lookup(|key| match key {
            "WISHPOOL_BIND" => Some("127.0.0.1:8080".into()),
            "WISHPOOL_PUBLIC_URL" => Some("http://127.0.0.1:5173".into()),
            "WISHPOOL_AUTH_MODE" => Some("dev".into()),
            "WISHPOOL_STORAGE" => Some("memory".into()),
            "WISHPOOL_TEX_BIN" => Some("/missing/mathesis-test-tex".into()),
            _ => None,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn diagnostics_distinguish_disabled_and_missing_execution_without_exposing_paths() {
        let (ready, status) = RuntimeReadiness::new(config(), None).check().await;
        assert!(!ready);
        assert_eq!(status["compile"], "unavailable");
        assert_eq!(status["cma"], "disabled");
        assert_eq!(status["automatic_review"], false);
        assert_eq!(status["provider_execution_verified"], false);
        assert!(!status.to_string().contains("mathesis-test-tex"));
    }

    #[tokio::test]
    async fn api_readiness_does_not_remove_read_access_when_execution_is_unavailable() {
        let mut config = config();
        config.role = crate::config::Role::Api;
        let (ready, status) = RuntimeReadiness::new(config, None).check().await;
        assert!(ready);
        assert_eq!(status["execution"], "worker_only");
    }
}
