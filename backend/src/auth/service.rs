use bcrypt::{hash, verify, DEFAULT_COST};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    auth::jwt,
    error::AppError,
    state::AppState,
};

async fn create_session(state: &AppState, user_id: Uuid) -> Result<Uuid, AppError> {
    let session = sqlx::query!(
        "INSERT INTO sessions (user_id) VALUES ($1) RETURNING id",
        user_id
    )
    .fetch_one(&state.db)
    .await?;
    Ok(session.id)
}

pub async fn update_email(state: Arc<AppState>, user_id: Uuid, new_email: &str) -> Result<(), AppError> {
    let email = new_email.trim().to_lowercase();
    if email.is_empty() {
        return Err(AppError::BadRequest("email cannot be empty".into()));
    }
    sqlx::query!("UPDATE users SET email = $1 WHERE id = $2", email, user_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn logout(state: Arc<AppState>, session_id: &str) -> Result<(), AppError> {
    let sid = Uuid::parse_str(session_id)
        .map_err(|_| AppError::BadRequest("invalid session id".into()))?;
    sqlx::query!("UPDATE sessions SET revoked = true WHERE id = $1", sid)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn register(
    state: Arc<AppState>,
    req: crate::auth::handlers::RegisterRequest,
) -> Result<(String, String, Uuid), AppError> {
    let email = req.email.trim().to_lowercase();

    if email.is_empty() || req.password.len() < 8 {
        return Err(AppError::BadRequest("invalid email or password".into()));
    }

    let password_hash = hash(&req.password, DEFAULT_COST)
        .map_err(|_| AppError::BadRequest("hash error".into()))?;

    let user = sqlx::query!(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING id, email",
        &email,
        password_hash
    )
    .fetch_one(&state.db)
    .await?;

    let session_id = create_session(&state, user.id).await?;

    let token = jwt::make_token(
        &user.id.to_string(),
        &email,
        &session_id.to_string(),
        &state.jwt_secret,
    )?;

    Ok((token, email, session_id))
}

pub async fn login(
    state: Arc<AppState>,
    req: crate::auth::handlers::LoginRequest,
) -> Result<(String, String, Uuid), AppError> {
    let email = req.email.trim().to_lowercase();

    let user = sqlx::query!(
        "SELECT id, email, password_hash FROM users WHERE email = $1",
        &email
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Auth("invalid credentials".into()))?;

    let ok = verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Auth("invalid credentials".into()))?;

    if !ok {
        return Err(AppError::Auth("invalid credentials".into()));
    }

    let session_id = create_session(&state, user.id).await?;

    let token = jwt::make_token(
        &user.id.to_string(),
        &email,
        &session_id.to_string(),
        &state.jwt_secret,
    )?;

    Ok((token, email, session_id))
}
