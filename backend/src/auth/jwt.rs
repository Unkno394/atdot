use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

use crate::{error::AppError, models::Claims};

pub fn make_token(
    user_id: &str,
    email: &str,
    session_id: &str,
    secret: &str,
) -> Result<String, AppError> {
    let exp = (Utc::now() + chrono::Duration::days(7)).timestamp() as usize;

    let claims = Claims {
        sub: user_id.into(),
        email: email.into(),
        session_id: session_id.into(),
        exp,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| AppError::Auth("token error".into()))
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::Auth("invalid token".into()))
}
