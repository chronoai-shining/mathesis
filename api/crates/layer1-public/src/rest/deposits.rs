use super::{ApiResult, AppState, Body, PageQuery, Path, Query};
use crate::auth::AuthenticatedCaller;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use std::sync::Arc;
use wishpool_core::{
    app::App,
    ids::{ClaimId, RecordId},
    model::NewLeanDeposit,
};
pub(super) fn routes() -> Router<Arc<App>> {
    Router::new()
        .route(
            "/formalizations",
            get(list)
                .post(submit)
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route("/formalizations/{id}", get(detail))
        .route("/formalizations/{id}/source", get(source))
        .route("/formalizations/{id}/attempts", post(verify))
}
async fn list(
    State(app): AppState,
    AuthenticatedCaller(c): AuthenticatedCaller,
    Query(q): Query<PageQuery>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(app.lean_deposits(&c, q.limit, q.before).await?))
}
async fn submit(
    State(app): AppState,
    AuthenticatedCaller(c): AuthenticatedCaller,
    Body(body): Body<NewLeanDeposit>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        StatusCode::CREATED,
        Json(app.submit_lean_deposit(&c, body).await?),
    ))
}
async fn detail(
    State(app): AppState,
    AuthenticatedCaller(c): AuthenticatedCaller,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    Ok(Json(app.lean_deposit(&c, &id).await?))
}
async fn source(
    State(app): AppState,
    AuthenticatedCaller(c): AuthenticatedCaller,
    Path(id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=Solution.lean",
            ),
            (header::CACHE_CONTROL, "private, no-store"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        app.lean_deposit_source(&c, &id).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    record: RecordId,
    claim: ClaimId,
}
async fn verify(
    State(app): AppState,
    AuthenticatedCaller(c): AuthenticatedCaller,
    Path(id): Path<String>,
    Body(target): Body<Target>,
) -> ApiResult<impl IntoResponse> {
    Ok((
        StatusCode::ACCEPTED,
        Json(
            app.verify_lean_deposit(&c, &id, &target.record, &target.claim)
                .await?,
        ),
    ))
}
