use axum::{routing::{delete, get, post}, Router};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

mod api;
mod auth;
mod error;
mod fraud;
mod models;
mod state;

use crate::fraud::scoring::{FraudEngineInner, FLUSH_INTERVAL};
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let jwt_secret = std::env::var("JWT_SECRET")
        .expect("JWT_SECRET must be set");
    let graphs_path = std::env::var("GRAPHS_DB_PATH")
        .unwrap_or_else(|_| "./data/graphs".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let fraud = FraudEngineInner::new(pool.clone(), &graphs_path)?;

    // Background worker: flush cold graphs from RAM to RocksDB every 30s
    let fraud_worker = fraud.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(FLUSH_INTERVAL);
        loop {
            interval.tick().await;
            fraud_worker.flush_cold_graphs();
        }
    });

    let (ws_tx, _) = tokio::sync::broadcast::channel::<String>(256);
    let state = Arc::new(AppState { db: pool, jwt_secret, fraud, ws_tx });

    let app = Router::new()
        // auth
        .route("/api/auth/register",  post(api::auth::register))
        .route("/api/auth/login",     post(api::auth::login))
        .route("/api/auth/logout",    post(api::auth::logout))
        .route("/api/auth/me",        get(api::auth::me).patch(api::auth::update_email))
        // api keys
        .route("/api/keys",           get(api::apikey::list).post(api::apikey::create))
        .route("/api/keys/:id",       delete(api::apikey::delete))
        // events
        .route("/api/ingest",          post(api::events::ingest))
        .route("/api/ingest/batch",   post(api::events::ingest_batch))
        .route("/api/stats",          get(api::events::stats))
        .route("/api/events/recent",  get(api::events::recent))
        .route("/api/feedback",        post(api::feedback::submit))
        .route("/api/challenge/verify", post(api::challenge::verify))
        // websocket
        .route("/ws",                 get(api::ws::handler))
        // debug (only for local dev, no auth)
        .route("/debug/stats",        get(api::debug::debug_stats))
        .route("/sdk/atdot.js",       get(serve_sdk))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("madrigal backend listening on :{port}");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn serve_sdk() -> impl axum::response::IntoResponse {
    let sdk = include_str!("../../atdot/public/sdk/atdot.js");
    ([("content-type", "application/javascript; charset=utf-8")], sdk)
}