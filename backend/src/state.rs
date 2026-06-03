use tokio::sync::broadcast;
use crate::fraud::FraudEngine;

pub struct AppState {
    pub db:         sqlx::PgPool,
    pub jwt_secret: String,
    pub fraud:      FraudEngine,
    pub ws_tx:      broadcast::Sender<String>,
}
