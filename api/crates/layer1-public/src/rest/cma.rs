use super::{ApiResult, AppState, Body};
use crate::AuthenticatedCaller;
use axum::extract::State;
use axum::{Json, Router, routing::get};
use mathesis_core::{
    app::App,
    model::{MyCma, SetCmaWorkspace},
};
use std::sync::Arc;
pub fn routes() -> Router<Arc<App>> {
    Router::new().route("/me/cma", get(status).put(workspace).delete(disconnect))
}
async fn status(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
) -> ApiResult<Json<MyCma>> {
    Ok(Json(app.my_cma(&caller).await?))
}
async fn workspace(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
    Body(input): Body<SetCmaWorkspace>,
) -> ApiResult<Json<MyCma>> {
    Ok(Json(app.set_cma_workspace(&caller, input).await?))
}
async fn disconnect(
    State(app): AppState,
    AuthenticatedCaller(caller): AuthenticatedCaller,
) -> ApiResult<Json<MyCma>> {
    app.disconnect_cma(&caller).await?;
    Ok(Json(app.my_cma(&caller).await?))
}
