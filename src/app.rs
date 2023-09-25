use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::{
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
    pub async fn build() {
        Self::setup_tracing("info");
        dotenv::dotenv().expect("Unable to load environment variables from .env file");
        let db_url = std::env::var("DATABASE_URL").expect("Unable to read DATABASE_URL env var");

        let db_pool = Self::get_pool(&db_url).await;
        let (tx, _rx) = broadcast::channel(100);
        let app_state = Arc::new(AppState {
            db_pool: db_pool.clone(),
            tx,
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
            .layer(cors);

        let addr = "0.0.0.0:8009".parse::<SocketAddr>().unwrap();
        tracing::info!("listening on {}", addr.port());
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
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

    async fn get_pool(db_url: &str) -> Db {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .connect(&db_url)
            .await
            .expect("can't connect to database");
        Db(pool)
    }
}
