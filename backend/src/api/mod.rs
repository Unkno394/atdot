pub mod auth;
pub mod apikey;
pub mod challenge;
pub mod events;
pub mod feedback;
pub mod debug;
pub mod ws;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use std::sync::Arc;

use crate::{auth::jwt, error::AppError, state::AppState};

pub struct AuthUser {
    pub user_id:    String,
    pub email:      String,
    pub session_id: String,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, AppError> {
        let token = parts.headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::Auth("missing or invalid Authorization header".into()))?;

        let claims = jwt::decode_token(token, &state.jwt_secret)?;
        Ok(AuthUser {
            user_id:    claims.sub,
            email:      claims.email,
            session_id: claims.session_id,
        })
    }
}
