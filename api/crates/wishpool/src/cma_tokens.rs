//! Reuse sealed delegated-token refresh; fall back to an operator-mounted user token.
use crate::donations::HostedWorker;
use async_trait::async_trait;
use std::{path::PathBuf, sync::Arc};
use wishpool_core::ids::PersonId;
use wishpool_review::{ReviewError, ReviewResult, cma::TokenSource};
pub struct CmaTokens {
    pub hosted: Option<Arc<HostedWorker>>,
    pub account: Option<PersonId>,
    pub file: Option<PathBuf>,
}
#[async_trait]
impl TokenSource for CmaTokens {
    async fn token(&self) -> ReviewResult<String> {
        if let (Some(hosted), Some(account)) = (&self.hosted, &self.account) {
            match hosted.access_token(account).await {
                Ok(token) => return Ok(token),
                Err(_) if self.file.is_some() => {
                    tracing::warn!("CMA delegated token unavailable; using configured token file")
                }
                Err(_) => {
                    return Err(ReviewError::Transport(
                        "CMA delegated token unavailable".into(),
                    ));
                }
            }
        }
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| ReviewError::Output("CMA user token is not configured".into()))?;
        if std::fs::metadata(file)
            .map_err(|_| ReviewError::Output("CMA token file unavailable".into()))?
            .len()
            > 64 * 1024
        {
            return Err(ReviewError::Output("CMA token file too large".into()));
        }
        std::fs::read_to_string(file)
            .map_err(|_| ReviewError::Output("CMA token file unavailable".into()))
    }
}

/// Creates the transport from a committed attempt; author credentials have no fallback source.
pub struct CmaFactory {
    pub operator: Arc<wishpool_review::cma::Client>,
    pub connection: Option<Arc<crate::cma_connection::CmaConnection>>,
    pub app: Arc<wishpool_core::app::App>,
}
impl CmaFactory {
    pub fn client(
        &self,
        attempt: &wishpool_core::model::CmaAttempt,
    ) -> ReviewResult<Arc<wishpool_review::cma::Client>> {
        use wishpool_core::model::CmaPayer;
        use wishpool_review::cma::{Client, HttpTransport};
        let transport = match attempt.payer {
            CmaPayer::Operator => self.operator.transport.clone(),
            CmaPayer::Author => {
                let connection = self.connection.as_ref().ok_or(ReviewError::CmaRefusal(
                    wishpool_review::cma::FailureKind::Auth,
                ))?;
                Arc::new(HttpTransport {
                    base_url: connection.base_url.clone(),
                    tokens: Arc::new(AuthorTokens {
                        connection: connection.clone(),
                        app: self.app.clone(),
                        person: attempt.owner.clone().ok_or_else(|| {
                            ReviewError::Output("CMA attempt owner missing".into())
                        })?,
                        generation: attempt.binding_generation.ok_or_else(|| {
                            ReviewError::Output("CMA attempt generation missing".into())
                        })?,
                    }),
                })
            }
        };
        Ok(Arc::new(Client {
            transport,
            workspace: attempt.workspace.clone(),
            // BYOK configuration pins a published profile readable by author accounts.
            profile: self.operator.profile.clone(),
            poll: self.operator.poll,
        }))
    }
}
struct AuthorTokens {
    app: Arc<wishpool_core::app::App>,
    connection: Arc<crate::cma_connection::CmaConnection>,
    person: PersonId,
    generation: u64,
}
#[async_trait]
impl TokenSource for AuthorTokens {
    async fn token(&self) -> ReviewResult<String> {
        let binding = self
            .app
            .cma_binding(&self.person)
            .await
            .map_err(|_| ReviewError::Transport("CMA binding unavailable".into()))?;
        if binding.generation != self.generation
            || binding.status == wishpool_core::model::CmaStatus::Disconnected
        {
            return Err(ReviewError::CmaRefusal(
                wishpool_review::cma::FailureKind::Auth,
            ));
        }
        let token = self
            .connection
            .access_token(&self.person, self.generation)
            .await
            .map_err(|e| match e {
                wishpool_core::CoreError::Unauthenticated => {
                    ReviewError::CmaRefusal(wishpool_review::cma::FailureKind::Auth)
                }
                _ => ReviewError::Transport("CMA author credential refresh unavailable".into()),
            })?;
        let binding = self
            .app
            .cma_binding(&self.person)
            .await
            .map_err(|_| ReviewError::Transport("CMA binding unavailable".into()))?;
        if binding.generation != self.generation
            || binding.status == wishpool_core::model::CmaStatus::Disconnected
            || self
                .connection
                .tokens
                .get(&crate::cma_connection::slot(&self.person, self.generation))
                .await
                .map_err(|_| ReviewError::Transport("CMA custody unavailable".into()))?
                .is_none()
        {
            return Err(ReviewError::CmaRefusal(
                wishpool_review::cma::FailureKind::Auth,
            ));
        }
        Ok(token)
    }
}
