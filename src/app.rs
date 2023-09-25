use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use crate::{
    config::{Config, DatabaseConfig},
    domain::events::AppEvent,
    routes::{
        auth::{authenticate, check_auth},
        event::stream,
        health,
        user::{get_authenticated_user, get_users},
    },
};
use axum::{
    middleware,
    routing::{get, post},
    Extension, Router,
};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct Db(Pool<Postgres>);

impl Db {
    pub fn inner(&self) -> Pool<Postgres> {
        self.0.clone()
    }
}

#[derive(Clone)]
pub struct AppState {
    db_pool: Db,
    tx: broadcast::Sender<AppEvent>,
    pub config: Config,
}

impl AppState {
    pub fn get_pool(&self) -> Pool<Postgres> {
        self.db_pool.inner()
    }

    pub fn get_sender(&self) -> broadcast::Sender<AppEvent> {
        self.tx.clone()
    }
}

pub struct Application;

impl Application {
    pub async fn build(config: Config) -> anyhow::Result<()> {
        Self::setup_tracing(&config.application.debug_mode);

        let db_pool = Self::get_pool(&config.database).await;
        let (tx, _rx) = broadcast::channel(100);
        let app_state = Arc::new(AppState {
            db_pool: db_pool.clone(),
            tx,
            config: config.clone(),
        });

        let cors = CorsLayer::permissive();
        let app = Router::new()
            .route("/stream", get(stream))
            .route("/users/me", get(get_authenticated_user))
            .route("/users", get(get_users))
            .route_layer(middleware::from_fn(check_auth))
            .route("/health", get(health))
            .route("/authenticate", post(authenticate))
            .with_state(app_state)
            .layer(Extension(db_pool.clone()))
            .layer(Extension(config.clone()))
            .layer(cors);

        let ip = config.application.host.parse::<IpAddr>()?;
        let addr = SocketAddr::new(ip, config.application.port);
        tracing::info!("listening on {}", addr.port());
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();

        Ok(())
    }

    fn setup_tracing(debug_mode: &str) {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| debug_mode.into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    async fn get_pool(db_config: &DatabaseConfig) -> Db {
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_lazy_with(db_config.get_connect_options());
        Db(pool)
    }
}
