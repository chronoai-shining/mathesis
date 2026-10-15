use super::{ApiResult, AppState, Body, Path};
use crate::auth::AuthenticatedCaller;
use axum::{
    Json, Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use mathesis_core::{
    app::App,
    ids::SubmissionId,
    model::{NewBindingBoard, NewBindingChallenge, NewBindingContribution},
};
use serde::Deserialize;
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/bindings", get(directory))
        .route("/binding-contributors/{person}", get(profile))
        .route("/bindings/{id}/{key}", get(read))
        .route("/bindings/{id}/{key}/contributions", post(contribute))
        .route("/bindings/{id}/{key}/challenges", post(challenge))
        .route("/submissions/{id}/bindings", get(private).post(publish))
        .route("/submissions/{id}/bindings/{key}/revoke", post(revoke))
}
#[derive(Deserialize)]
struct Page {
    limit: Option<u32>,
    before: Option<String>,
}
async fn directory(State(app): AppState, Query(page): Query<Page>) -> ApiResult<impl IntoResponse> {
    Ok(Json(app.binding_directory(page.limit, page.before).await?))
}
async fn read(
    State(app): AppState,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.public_binding_board(&SubmissionId(id), &key).await?,
    ))
}
async fn private(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.private_bindings(&caller, &SubmissionId(id)).await?,
    ))
}
async fn publish(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
    Body(input): Body<NewBindingBoard>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.publish_binding_board(&caller, &SubmissionId(id), input)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revoke {
    reason: String,
}
async fn revoke(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<Revoke>,
) -> ApiResult<impl IntoResponse> {
    app.revoke_binding_board(&caller, &SubmissionId(id), &key, input.reason)
        .await?;
    Ok(Json(serde_json::json!({"revoked":true})))
}
async fn contribute(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<NewBindingContribution>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.contribute_binding(&caller, &SubmissionId(id), &key, input)
            .await?,
    ))
}
async fn challenge(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<NewBindingChallenge>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.challenge_binding(&caller, &SubmissionId(id), &key, input)
            .await?,
    ))
}

async fn profile(
    State(app): AppState,
    Path(person): Path<String>,
    Query(page): Query<Page>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.binding_profile(&mathesis_core::ids::PersonId(person), page.before)
            .await?,
    ))
}
