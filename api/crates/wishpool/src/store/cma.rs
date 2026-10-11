use super::{MongoStore, from_document, to_document, unavailable, write_error};
use async_trait::async_trait;
use mongodb::bson::doc;
use wishpool_core::{
    CoreError, CoreResult, ids::PersonId, model::CmaBinding, ports::CmaBindingStore,
};

#[async_trait]
impl CmaBindingStore for MongoStore {
    async fn get(&self, person: &PersonId) -> CoreResult<Option<CmaBinding>> {
        self.raw("cma_bindings")
            .find_one(doc! {"person": person.as_str()})
            .await
            .map_err(unavailable)?
            .map(from_document)
            .transpose()
    }
    async fn replace(&self, binding: &CmaBinding, expected: u64) -> CoreResult<()> {
        if expected == 0 {
            self.raw("cma_bindings")
                .insert_one(to_document(binding)?)
                .await
                .map_err(write_error)?;
        } else {
            let result = self
                .raw("cma_bindings")
                .replace_one(
                    doc! {"person": binding.person.as_str(), "revision": expected as i64},
                    to_document(binding)?,
                )
                .await
                .map_err(write_error)?;
            if result.matched_count != 1 {
                return Err(CoreError::StaleRevision {
                    kind: "CMA binding",
                    id: binding.person.to_string(),
                });
            }
        }
        Ok(())
    }
}
