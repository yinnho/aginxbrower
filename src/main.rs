use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

mod browser;
mod config;
mod cookie;
mod error;
mod page;
mod server;

// Inlined Obscura engine (formerly external crates).
mod obscura_dom;
mod obscura_net;
mod obscura_js;
mod obscura_browser;

use server::{do_click, do_eval, do_fetch};

#[derive(Debug, Deserialize)]
pub struct FetchRequest {
    pub url: String,
    #[serde(default)]
    pub format: OutputFormat,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub wait_secs: Option<u64>,
    /// Route through OBSCURA_PROXY. Default false (direct) — set true for
    /// foreign sites that are blocked or slow without a proxy.
    #[serde(default)]
    pub use_proxy: bool,
    /// Cookies to inject before navigation (`["name=value", ...]`). For sites
    /// that gate content behind a logged-in session (e.g. WeChat articles).
    #[serde(default)]
    pub cookies: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Html,
    Text,
}

#[derive(Debug, Deserialize)]
pub struct ClickRequest {
    pub url: String,
    pub selector: String,
    #[serde(default)]
    pub wait_secs: Option<u64>,
    /// Route through OBSCURA_PROXY. Default false (direct).
    #[serde(default)]
    pub use_proxy: bool,
    /// Cookies to inject before navigation.
    #[serde(default)]
    pub cookies: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct EvalRequest {
    pub url: String,
    pub script: String,
    #[serde(default)]
    pub wait_secs: Option<u64>,
    /// Route through OBSCURA_PROXY. Default false (direct).
    #[serde(default)]
    pub use_proxy: bool,
    /// Cookies to inject before navigation.
    #[serde(default)]
    pub cookies: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FetchResponse {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ClickResponse {
    pub url: String,
    pub selector: String,
    pub clicked: bool,
    pub text_after: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EvalResponse {
    pub url: String,
    pub result: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub enum AppError {
    BadRequest(String),
    NotFound(String),
    BadGateway(String),
    GatewayTimeout(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadGateway(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::GatewayTimeout(msg) => (StatusCode::GATEWAY_TIMEOUT, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        let e = err.into();
        let msg = e.to_string();
        if msg.contains("timeout") || msg.contains("timed out") {
            AppError::GatewayTimeout(msg)
        } else if msg.contains("resolve") || msg.contains("connect") || msg.contains("dns") {
            AppError::BadGateway(msg)
        } else if msg.contains("selector") || msg.contains("parse") {
            AppError::BadRequest(msg)
        } else {
            AppError::Internal(msg)
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/fetch", post(fetch_handler))
        .route("/click", post(click_handler))
        .route("/eval", post(eval_handler));

    let bind_addr = std::env::var("AGINXBROWER_BIND").unwrap_or_else(|_| "0.0.0.0:8089".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("aginxbrower listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "engine": "obscura" }))
}

async fn fetch_handler(Json(req): Json<FetchRequest>) -> Result<impl IntoResponse, AppError> {
    let resp = spawn_blocking(move || do_fetch(req)).await?;
    Ok((StatusCode::OK, Json(resp?)))
}

async fn click_handler(Json(req): Json<ClickRequest>) -> Result<impl IntoResponse, AppError> {
    let resp = spawn_blocking(move || do_click(req)).await?;
    Ok((StatusCode::OK, Json(resp?)))
}

async fn eval_handler(Json(req): Json<EvalRequest>) -> Result<impl IntoResponse, AppError> {
    let resp = spawn_blocking(move || do_eval(req)).await?;
    Ok((StatusCode::OK, Json(resp?)))
}

fn spawn_blocking<F, R>(f: F) -> tokio::task::JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tokio::task::spawn_blocking(f)
}
