//! Donated quota attribution for venue CMA agent work. Credentials stay sealed.
//! CMA v2 currently refuses restricted delegated tokens; accounting awaits real
//! NyxID receipts. The maintenance loop only removes revoked credentials.

use std::{collections::HashMap, sync::Arc, time::Duration};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, AeadCore, OsRng},
};
use async_trait::async_trait;
use base64::Engine as _;
use mongodb::bson::doc;
use tokio::sync::{Mutex, watch};
use wishpool_core::{CoreError, CoreResult, app::App, ids::PersonId};

use crate::{
    auth::nyxid::NyxIdClient,
    store::{MongoStore, unavailable, write_error},
};

/// AES-256-GCM over the delegated refresh token.
#[derive(Clone)]
pub struct TokenCipher {
    cipher: Aes256Gcm,
}

impl TokenCipher {
    /// `key` is 32 bytes, standard base64.
    pub fn from_base64(key: &str) -> anyhow::Result<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(key.trim())?;
        anyhow::ensure!(
            bytes.len() == 32,
            "WISHPOOL_TOKEN_KEY must decode to 32 bytes"
        );
        Ok(Self {
            cipher: Aes256Gcm::new_from_slice(&bytes)?,
        })
    }

    pub fn seal(&self, plaintext: &str) -> CoreResult<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| CoreError::Unavailable("encryption failed".into()))?;
        let mut sealed = nonce.to_vec();
        sealed.extend(ciphertext);
        Ok(base64::engine::general_purpose::STANDARD.encode(sealed))
    }

    pub fn open(&self, sealed: &str) -> CoreResult<String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(sealed)
            .map_err(|_| CoreError::Unavailable("sealed token is not base64".into()))?;
        if bytes.len() < 13 {
            return Err(CoreError::Unavailable("sealed token too short".into()));
        }
        let (nonce, ciphertext) = bytes.split_at(12);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| {
                CoreError::Unavailable("sealed token does not open with this key".into())
            })?;
        String::from_utf8(plaintext)
            .map_err(|_| CoreError::Unavailable("sealed token is not UTF-8".into()))
    }
}

/// Sealed refresh tokens by donor.
#[async_trait]
pub trait DonorTokens: Send + Sync {
    async fn put(&self, donor: &PersonId, sealed: &str) -> CoreResult<()>;
    async fn get(&self, donor: &PersonId) -> CoreResult<Option<String>>;
    async fn delete(&self, donor: &PersonId) -> CoreResult<()>;
}

#[derive(Default)]
pub struct MemoryDonorTokens(Mutex<HashMap<PersonId, String>>);

#[async_trait]
impl DonorTokens for MemoryDonorTokens {
    async fn put(&self, donor: &PersonId, sealed: &str) -> CoreResult<()> {
        self.0.lock().await.insert(donor.clone(), sealed.to_owned());
        Ok(())
    }
    async fn get(&self, donor: &PersonId) -> CoreResult<Option<String>> {
        Ok(self.0.lock().await.get(donor).cloned())
    }
    async fn delete(&self, donor: &PersonId) -> CoreResult<()> {
        self.0.lock().await.remove(donor);
        Ok(())
    }
}

const DONOR_TOKENS: &str = "donor_tokens";

#[async_trait]
impl DonorTokens for MongoStore {
    async fn put(&self, donor: &PersonId, sealed: &str) -> CoreResult<()> {
        self.raw(DONOR_TOKENS)
            .replace_one(
                doc! { "_id": donor.as_str() },
                doc! { "_id": donor.as_str(), "sealed": sealed },
            )
            .upsert(true)
            .await
            .map_err(write_error)?;
        Ok(())
    }
    async fn get(&self, donor: &PersonId) -> CoreResult<Option<String>> {
        let found = self
            .raw(DONOR_TOKENS)
            .find_one(doc! { "_id": donor.as_str() })
            .await
            .map_err(unavailable)?;
        Ok(found.and_then(|d| d.get_str("sealed").ok().map(str::to_owned)))
    }
    async fn delete(&self, donor: &PersonId) -> CoreResult<()> {
        self.raw(DONOR_TOKENS)
            .delete_one(doc! { "_id": donor.as_str() })
            .await
            .map_err(unavailable)?;
        Ok(())
    }
}

/// Everything the donation flow and the hosted worker share.
pub struct Donations {
    pub tokens: Arc<dyn DonorTokens>,
    pub cipher: TokenCipher,
    pub scope: String,
    pub service_ids: Vec<String>,
    pub default_model: String,
}

/// Models a donor may choose; the gateway routes them by name prefix.
pub fn valid_model(model: &str) -> bool {
    (3..=100).contains(&model.len())
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '/'))
}

pub struct HostedWorker {
    pub app: Arc<App>,
    pub nyxid: Arc<NyxIdClient>,
    pub donations: Arc<Donations>,
    pub interval: Duration,
}

impl HostedWorker {
    pub async fn run(self: Arc<Self>, mut shutdown: watch::Receiver<bool>) {
        tracing::info!(
            "Donations attribute venue CMA runs; restricted delegated tokens cannot execute CMA v2. Usage awaits NyxID receipts."
        );
        loop {
            if *shutdown.borrow() {
                break;
            }
            // Rotation is demand-driven in CmaTokens. No legacy hosted judgement jobs.
            // Revoke also destroys the sealed credential; no quota is spent by this loop.
            for grant in self.app.revoked_donations().await.unwrap_or_default() {
                let _ = self.donations.tokens.delete(&grant).await;
            }
            tokio::select! { _ = tokio::time::sleep(self.interval) => {}, _ = shutdown.changed() => break }
        }
    }
    pub(crate) async fn access_token(&self, donor: &PersonId) -> CoreResult<String> {
        let sealed = self
            .donations
            .tokens
            .get(donor)
            .await?
            .ok_or(CoreError::Unauthenticated)?;
        let refresh = self.donations.cipher.open(&sealed)?;
        let tokens = self.nyxid.refresh(&refresh).await?;
        if let Some(rotated) = &tokens.refresh_token {
            self.donations
                .tokens
                .put(donor, &self.donations.cipher.seal(rotated)?)
                .await?;
        }
        Ok(tokens.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_tokens_round_trip_and_resist_other_keys() {
        let key = base64::engine::general_purpose::STANDARD.encode([7_u8; 32]);
        let other = base64::engine::general_purpose::STANDARD.encode([8_u8; 32]);
        let cipher = TokenCipher::from_base64(&key).unwrap();
        let sealed = cipher.seal("refresh-token").unwrap();
        assert_ne!(
            sealed,
            cipher.seal("refresh-token").unwrap(),
            "fresh nonce every time"
        );
        assert_eq!(cipher.open(&sealed).unwrap(), "refresh-token");
        assert!(
            TokenCipher::from_base64(&other)
                .unwrap()
                .open(&sealed)
                .is_err()
        );
        assert!(TokenCipher::from_base64("c2hvcnQ=").is_err());
    }

    #[test]
    fn model_names() {
        assert!(valid_model("claude-opus-5-5"));
        assert!(!valid_model("x"));
        assert!(!valid_model("rm -rf"));
    }
}
