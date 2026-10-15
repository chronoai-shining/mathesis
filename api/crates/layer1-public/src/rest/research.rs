//! Private research resources. Layer 2 owns every authorization and state check.
use super::{ApiResult, AppState, Body, Path};
use crate::auth::{AuthenticatedCaller, RequestAuthentication};
use axum::{
    Json, Router,
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use mathesis_core::{
    app::App,
    ids::SubmissionId,
    model::{NewResearchObligation, ResearchAttemptInput},
};
use serde::Deserialize;
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/submissions/{id}/research", get(read).post(adopt))
        .route("/submissions/{id}/research/{key}/stop", post(stop))
        .route(
            "/submissions/{id}/research/{key}/target/response",
            post(respond),
        )
        .route("/submissions/{id}/research/{key}/attempts", post(attempt))
        .route(
            "/submissions/{id}/research/{key}/attempts/{attempt}",
            get(read_attempt),
        )
        .route(
            "/submissions/{id}/research/{key}/attempts/{attempt}/recheck",
            post(recheck),
        )
}
async fn read(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.research_obligations(&caller, &SubmissionId(id)).await?,
    ))
}
async fn adopt(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path(id): Path<String>,
    Body(input): Body<NewResearchObligation>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.adopt_research_obligation(&caller, &SubmissionId(id), input)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stop {
    reason: String,
}
async fn stop(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<Stop>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.stop_research_obligation(&caller, &SubmissionId(id), &key, input.reason)
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    digest: String,
    confirm: bool,
    #[serde(default)]
    comment: String,
}
async fn respond(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    RequestAuthentication(authentication): RequestAuthentication,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<Response>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.respond_research_target(
            &caller,
            authentication,
            &SubmissionId(id),
            &key,
            &input.digest,
            input.confirm,
            input.comment,
        )
        .await?,
    ))
}
async fn attempt(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key)): Path<(String, String)>,
    Body(input): Body<ResearchAttemptInput>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.submit_research_attempt(&caller, &SubmissionId(id), &key, input)
            .await?,
    ))
}
async fn read_attempt(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key, attempt)): Path<(String, String, String)>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(
        app.research_attempt(&caller, &SubmissionId(id), &key, &attempt)
            .await?,
    ))
}
async fn recheck(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Path((id, key, attempt)): Path<(String, String, String)>,
) -> ApiResult<impl IntoResponse> {
    app.recheck_research(&caller, &SubmissionId(id), &key, &attempt)
        .await?;
    Ok(Json(serde_json::json!({"requested":true})))
}
