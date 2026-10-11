//! Ordinary NyxID consent and isolated, sealed CMA refresh-token custody.
use crate::{
    auth::nyxid::NyxIdClient,
    donations::{DonorTokens, TokenCipher, refresh_access_token},
    store::{MongoStore, unavailable, write_error},
};
use async_trait::async_trait;
use mongodb::bson::doc;
use std::sync::Arc;
use wishpool_core::{
    CoreError, CoreResult,
    ids::PersonId,
    model::{CmaBinding, CmaProfile, CmaWorkspaceCheck, CmaWorkspaceFacts},
    ports::CmaSetup,
};
use wishpool_review::cma::{HttpTransport, TokenSource};

pub struct CmaSetupAdapter(pub Arc<CmaConnection>);

impl std::ops::Deref for CmaSetupAdapter {
    type Target = Arc<CmaConnection>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct CmaConnection {
    pub tokens: Arc<dyn DonorTokens>,
    pub cipher: TokenCipher,
    pub nyxid: Arc<NyxIdClient>,
    pub base_url: String,
    pub public_url: String,
}
pub fn slot(person: &PersonId, generation: u64) -> PersonId {
    PersonId(format!("cma_review:{}:{generation}", person.as_str()))
}
impl CmaConnection {
    pub async fn access_token(&self, person: &PersonId, generation: u64) -> CoreResult<String> {
        refresh_access_token(
            &self.nyxid,
            self.tokens.as_ref(),
            &self.cipher,
            &slot(person, generation),
        )
        .await
    }
}

/// Separate collection: CMA consent never replaces a donated-quota grant.
pub struct MongoCmaTokens(pub MongoStore);
#[async_trait]
impl DonorTokens for MongoCmaTokens {
    async fn put(&self, id: &PersonId, sealed: &str) -> CoreResult<()> {
        self.0
            .raw("cma_tokens")
            .replace_one(
                doc! {"_id": id.as_str()},
                doc! {"_id":id.as_str(), "sealed":sealed},
            )
            .upsert(true)
            .await
            .map_err(write_error)?;
        Ok(())
    }
    async fn get(&self, id: &PersonId) -> CoreResult<Option<String>> {
        Ok(self
            .0
            .raw("cma_tokens")
            .find_one(doc! {"_id":id.as_str()})
            .await
            .map_err(unavailable)?
            .and_then(|d| d.get_str("sealed").ok().map(str::to_owned)))
    }
    async fn delete(&self, id: &PersonId) -> CoreResult<()> {
        self.0
            .raw("cma_tokens")
            .delete_one(doc! {"_id":id.as_str()})
            .await
            .map_err(unavailable)?;
        Ok(())
    }
    async fn rotate(&self, id: &PersonId, expected: &str, sealed: &str) -> CoreResult<()> {
        let result = self
            .0
            .raw("cma_tokens")
            .update_one(
                doc! {"_id":id.as_str(), "sealed":expected},
                doc! {"$set":{"sealed":sealed}},
            )
            .await
            .map_err(write_error)?;
        if result.matched_count != 1 {
            return Err(CoreError::conflict("CMA credential rotation raced"));
        }
        Ok(())
    }
}
struct ValidationTokens {
    connection: Arc<CmaConnection>,
    person: PersonId,
    generation: u64,
}
#[async_trait]
impl TokenSource for ValidationTokens {
    async fn token(&self) -> wishpool_review::ReviewResult<String> {
        self.connection
            .access_token(&self.person, self.generation)
            .await
            .map_err(|e| match e {
                CoreError::Unauthenticated => wishpool_review::ReviewError::Provider {
                    status: 401,
                    body: "not_authenticated".into(),
                },
                _ => wishpool_review::ReviewError::Transport(
                    "CMA credential refresh unavailable".into(),
                ),
            })
    }
}

#[async_trait]
impl CmaSetup for CmaSetupAdapter {
    fn enabled(&self) -> bool {
        true
    }
    fn setup_url(&self) -> String {
        format!(
            "{}/auth/login?return_to=/",
            self.public_url.trim_end_matches('/')
        )
    }
    async fn revoke(&self, person: &PersonId, generation: u64) -> CoreResult<()> {
        self.tokens.delete(&slot(person, generation)).await
    }
    async fn validate(
        &self,
        binding: &CmaBinding,
        workspace: &str,
    ) -> CoreResult<CmaWorkspaceCheck> {
        let transport = HttpTransport {
            base_url: self.base_url.clone(),
            tokens: Arc::new(ValidationTokens {
                connection: self.0.clone(),
                person: binding.person.clone(),
                generation: binding.generation,
            }),
        };
        match wishpool_review::cma::resources::workspace_facts(&transport, workspace).await {
            Ok(f) => Ok(CmaWorkspaceCheck::Resources(CmaWorkspaceFacts {
                sandbox_id: f.sandbox_id,
                sandbox_profile: CmaProfile {
                    id: f.sandbox_profile.id,
                    revision: f.sandbox_profile.revision,
                },
                owner: f.owner,
                workspace_status: f.workspace_status,
                sandbox_status: f.sandbox_status,
                model_access: f.model_access,
            })),
            Err(wishpool_review::ReviewError::Provider {
                status: 401 | 403, ..
            }) => Ok(CmaWorkspaceCheck::AuthorizationRefused),
            Err(wishpool_review::ReviewError::Provider {
                status: 404 | 409 | 422,
                ..
            })
            | Err(wishpool_review::ReviewError::Output(_)) => Ok(CmaWorkspaceCheck::SetupMissing),
            Err(_) => Err(CoreError::Unavailable(
                "CMA validation unavailable; try again".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donations::MemoryDonorTokens;
    #[tokio::test]
    async fn rotating_a_deleted_generation_cannot_resurrect_it() {
        let tokens = MemoryDonorTokens::default();
        let person = PersonId::from("author");
        let old = slot(&person, 1);
        let next = slot(&person, 2);
        let value = crate::auth::random_token();
        tokens.put(&old, &value).await.unwrap();
        tokens.put(&next, &value).await.unwrap();
        tokens.delete(&old).await.unwrap();
        assert!(
            tokens
                .rotate(&old, &value, &crate::auth::random_token())
                .await
                .is_err()
        );
        assert!(tokens.get(&old).await.unwrap().is_none());
        assert!(tokens.get(&next).await.unwrap().is_some());
    }
}
