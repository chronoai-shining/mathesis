use super::{ApiResult, AppState, Body, Path};
use crate::auth::AuthenticatedCaller;
use axum::{
    Json, Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use mathesis_core::{
    app::App,
    ids::SubmissionId,
    model::{FeedbackAuthorization, FeedbackKind, FeedbackReadbackRequest, NewFeedbackProposal},
};
use serde::Deserialize;
use std::sync::Arc;
pub(super) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/submissions/{id}/feedback", get(list).post(prepare))
        .route("/submissions/{id}/feedback/preview", post(preview))
        .route("/submissions/{id}/feedback/{key}", get(read))
        .route(
            "/submissions/{id}/feedback/{key}/observations/{request}",
            get(evidence),
        )
        .route(
            "/submissions/{id}/feedback/{key}/authorize",
            post(authorize),
        )
        .route("/submissions/{id}/feedback/{key}/revoke", post(revoke))
        .route("/submissions/{id}/feedback/{key}/export", post(export))
        .route("/submissions/{id}/feedback/{key}/readback", post(readback))
}
async fn list(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.feedback_proposals(&caller, &SubmissionId(id)).await?,
    ))
}
async fn preview(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
    Body(input): Body<FeedbackKind>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.preview_feedback(&caller, &SubmissionId(id), input)
            .await?,
    ))
}
async fn prepare(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
    Body(input): Body<NewFeedbackProposal>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.prepare_feedback(&caller, &SubmissionId(id), input)
            .await?,
    ))
}
async fn read(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(app.feedback(&caller, &SubmissionId(id), &key).await?))
}
async fn authorize(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<FeedbackAuthorization>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.authorize_feedback(&caller, &SubmissionId(id), &key, input)
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
    app.revoke_feedback(&caller, &SubmissionId(id), &key, input.reason)
        .await?;
    Ok(Json(serde_json::json!({"revoked":true})))
}
async fn export(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.export_feedback(&caller, &SubmissionId(id), &key)
            .await?,
    ))
}
async fn readback(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<FeedbackReadbackRequest>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.request_feedback_readback(&caller, &SubmissionId(id), &key, input)
            .await?,
    ))
}

async fn evidence(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key, request)): Path<(String, String, String)>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.feedback_observation_evidence(&caller, &SubmissionId(id), &key, &request)
            .await?,
    ))
}
