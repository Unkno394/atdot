
use axum::{extract::State, http::HeaderMap, Json};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    auth::{jwt, service},
    error::AppError,
    state::AppState,
};

#[derive(serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub email: String,
    pub session_id: Uuid,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let (token, email, session_id) = service::register(state, req).await?;
    Ok(Json(AuthResponse { token, email, session_id }))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let (token, email, session_id) = service::login(state, req).await?;
    Ok(Json(AuthResponse { token, email, session_id }))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Auth("missing token".into()))?;

    let claims = jwt::decode_token(token, &state.jwt_secret)?;
    service::logout(state, &claims.session_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Serialize)]
pub struct MeResponse {
    pub user_id:    String,
    pub email:      String,
    pub session_id: String,
}

#[derive(serde::Deserialize)]
pub struct UpdateEmailRequest {
    pub email: String,
}

pub async fn update_email(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<UpdateEmailRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Auth("missing token".into()))?;

    let claims = jwt::decode_token(token, &state.jwt_secret)?;
    let uid = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("invalid user id".into()))?;

    service::update_email(state, uid, &req.email).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AppError> {
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Auth("missing token".into()))?;

    let claims = jwt::decode_token(token, &state.jwt_secret)?;
    Ok(Json(MeResponse {
        user_id:    claims.sub,
        email:      claims.email,
        session_id: claims.session_id,
    }))
}
