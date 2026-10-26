//! PostgreSQL control plane. Enrollment acknowledges registration, not a node-enforced VPN lease.
pub mod auth;
pub mod config;
pub mod db;
pub mod devices;
pub mod servers;
pub mod settings;
pub mod subscriptions;
pub mod time;
pub mod users;
use axum::Extension;
use time::{Anchor, VerifiedTime};

use axum::{
    Json, Router,
    extract::{FromRequest, Request, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    bootstrap_secret: Arc<String>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    time_anchor: Arc<Mutex<Anchor>>,
}
impl AppState {
    pub async fn connect(
        url: &str,
        bootstrap_secret: String,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let pool = PgPoolOptions::new().max_connections(8).connect(url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        let clock: Arc<dyn Fn() -> i64 + Send + Sync> = Arc::new(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|v| v.as_secs() as i64)
                .unwrap_or(-1)
        });
        let time_anchor = Arc::new(Mutex::new(Anchor::new(clock())));
        Ok(Self {
            pool,
            bootstrap_secret: Arc::new(bootstrap_secret),
            clock,
            time_anchor,
        })
    }
    pub fn with_clock(mut self, clock: impl Fn() -> i64 + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self.time_anchor = Arc::new(Mutex::new(Anchor::new((self.clock)())));
        self
    }
    pub fn now(&self) -> i64 {
        (self.clock)()
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}
impl ApiError {
    pub fn bad(message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_input",
            message,
        }
    }
    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "Invalid or expired credentials",
        }
    }
    pub fn conflict(message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "conflict",
            message,
        }
    }
    pub fn missing() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "Resource not found",
        }
    }
    pub fn clock_untrusted() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "clock_untrusted",
            message: "Authorization paused: correct server clock, then sign in again",
        }
    }
    pub fn internal() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal",
            message: "Storage or entropy unavailable",
        }
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => Self::missing(),
            sqlx::Error::Database(ref d) if d.is_foreign_key_violation() => {
                Self::bad("Referenced resource does not exist")
            }
            _ => Self::internal(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.code,"message":self.message}})),
        )
            .into_response()
    }
}
pub struct ApiJson<T>(pub T);
impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for ApiJson<T> {
    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(v)| Self(v))
            .map_err(|_| ApiError::bad("Invalid JSON request"))
    }
    type Rejection = ApiError;
}
pub struct ApiId(pub uuid::Uuid);
impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ApiId {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        axum::extract::Path::<uuid::Uuid>::from_request_parts(parts, state)
            .await
            .map(|axum::extract::Path(v)| Self(v))
            .map_err(|_| ApiError::bad("Invalid resource identifier"))
    }
}
pub type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn router(state: AppState) -> Router {
    let admin = Router::new()
        .route("/v1/users", get(users::list).post(users::create))
        .route("/v1/users/{id}", patch(users::update).delete(users::remove))
        .route(
            "/v1/subscriptions",
            get(subscriptions::list).post(subscriptions::create),
        )
        .route("/v1/subscriptions/{id}", patch(subscriptions::update))
        .route("/v1/subscriptions/{id}/revoke", post(subscriptions::revoke))
        .route("/v1/subscriptions/{id}/rotate", post(subscriptions::rotate))
        .route("/v1/devices", get(devices::list))
        .route("/v1/devices/{id}", delete(devices::revoke))
        .route("/v1/servers", get(servers::list).post(servers::create))
        .route("/v1/overview", get(overview))
        .route(
            "/v1/settings",
            get(settings::get_settings).put(settings::put_settings),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));
    Router::new()
        .merge(admin)
        .route("/v1/health", get(health))
        .route("/v1/admin/bootstrap", post(auth::bootstrap))
        .route("/v1/admin/session", post(auth::session))
        .route("/v1/enroll/challenge", post(devices::challenge))
        .route("/v1/enroll", post(devices::enroll))
        .fallback(|| async { ApiError::missing() })
        .layer(middleware::from_fn_with_state(
            state.clone(),
            time::require_trusted_time,
        ))
        .with_state(state)
}
async fn health(State(s): State<AppState>) -> ApiResult<Value> {
    sqlx::query("SELECT 1").execute(&s.pool).await?;
    Ok(Json(
        json!({"status":"ok","version":env!("CARGO_PKG_VERSION"),"storage":"postgresql"}),
    ))
}
async fn overview(State(s): State<AppState>) -> ApiResult<Value> {
    let row: (i64,i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM users),(SELECT count(*) FROM subscriptions),(SELECT count(*) FROM devices WHERE status='active'),(SELECT count(*) FROM servers)").fetch_one(&s.pool).await?;
    Ok(Json(
        json!({"users":row.0,"subscriptions":row.1,"devices":row.2,"servers":row.3}),
    ))
}
#[derive(Serialize, sqlx::FromRow)]
pub struct User {
    pub id: uuid::Uuid,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub email: String,
    pub status: String,
    pub created_at: i64,
}
#[derive(Serialize, sqlx::FromRow)]
pub struct Subscription {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub status: String,
    pub expires_at: i64,
    pub device_limit: i64,
    pub created_at: i64,
}
