use std::{
    convert::Infallible,
    fmt::{Debug, Display},
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_stream::try_stream;
use axum::{
    extract::State,
    headers::{authorization::Bearer, Authorization},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{
        sse::{Event, KeepAlive},
        IntoResponse, Response, Sse,
    },
    routing::{get, post},
    Extension, Json, Router, TypedHeader,
};
use futures::stream::Stream;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::{distributions::Uniform, prelude::Distribution};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool, Pool, Postgres};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Serialize, Clone)]
struct NewReferralEvent {
    referrer: Username,
    referred_user: Username,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", content = "data")]
enum AppEvent {
    NewLogin(User),
    NewRegister(User),
    NewReferral(NewReferralEvent),
}

#[derive(Clone)]
struct Db(Pool<Postgres>);

#[derive(Clone)]
struct AppState {
    db_pool: Db,

    tx: broadcast::Sender<AppEvent>,
}

#[derive(Deserialize)]
struct AuthenticateRequest {
    username: Username,
    invitation_code: Option<InviteCode>,
}

#[derive(Serialize)]
struct AuthenticateResponse {
    username: String,
    invite_code: String,
    referrals: u32,
    token: String,
}

impl From<(User, u32, String)> for AuthenticateResponse {
    fn from((user, referrals, token): (User, u32, String)) -> Self {
        Self {
            username: user.username.inner(),
            invite_code: user.invite_code.inner(),
            referrals,
            token,
        }
    }
}

enum DatabaseError {
    ServerError,
}

enum ApiError {
    InvalidInviteCode,
    ServerError,
    AuthenticationError,
}

impl From<DatabaseError> for ApiError {
    fn from(value: DatabaseError) -> Self {
        match value {
            DatabaseError::ServerError => Self::ServerError,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            Self::InvalidInviteCode => (StatusCode::BAD_REQUEST, "Invalid invite code"),
            Self::ServerError => (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong"),
            Self::AuthenticationError => (StatusCode::UNAUTHORIZED, "Authentication failed"),
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv::dotenv().expect("Unable to load environment variables from .env file");

    let db_url = std::env::var("DATABASE_URL").expect("Unable to read DATABASE_URL env var");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&db_url)
        .await
        .expect("can't connect to database");
    let pool = Db(pool);

    let cors = CorsLayer::new().allow_origin(Any);
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState {
        db_pool: pool.clone(),
        tx,
    });

    let app = Router::new()
        .route("/stream", get(stream))
        .route_layer(middleware::from_fn(check_auth))
        .route("/health", get(health))
        .route("/authenticate", post(authenticate))
        .with_state(app_state)
        .layer(cors)
        .layer(Extension(pool.clone()));

    let addr = "0.0.0.0:8009".parse::<SocketAddr>().unwrap();
    tracing::info!("listening on {}", addr.port());
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn authenticate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AuthenticateRequest>,
) -> Result<Json<AuthenticateResponse>, ApiError> {
    let pool = state.db_pool.clone().0;
    tracing::info!("authenticating user >>> {}", payload.username);
    let user = get_user_by_username(&pool, &payload.username).await?;

    if let Some(user) = user {
        let referrals = get_user_referral_count(&pool, &user.username).await?;
        let _ = state.tx.send(AppEvent::NewLogin(user.clone()));
        let token = generate_auth_token(&user.username)?;
        return Ok(Json((user, referrals, token).into()));
    }

    let referrer_id = if let Some(invite_code) = payload.invitation_code {
        get_user_by_invite_code(&pool, &invite_code)
            .await
            .map_err(|_| ApiError::InvalidInviteCode)?
            .map(|u| u.username)
    } else {
        None
    };

    let invite_code = {
        let username = payload.username.as_ref();
        let mut code = InviteCode::new(username);
        while get_user_by_invite_code(&pool, &code).await?.is_some() {
            code = InviteCode::new(username);
        }
        code
    };

    let user = create_new_user(&pool, &payload.username, &invite_code, referrer_id).await?;
    if user.referred_by.is_some() {
        let _ = state.tx.send(AppEvent::NewReferral(NewReferralEvent {
            referred_user: user.clone().username,
            referrer: user.clone().referred_by.unwrap(),
        }));
    }

    let referrals = get_user_referral_count(&pool, &user.username).await?;

    let _ = state.tx.send(AppEvent::NewRegister(user.clone()));
    let token = generate_auth_token(&user.username)?;
    Ok(Json((user, referrals, token).into()))
}

async fn health() -> Json<Value> {
    Json(json!( {
        "message": "API up!",
    }))
}

async fn stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::info!("new connection to sse stream >>>");

    let mut rx = state.tx.clone().subscribe();

    Sse::new(try_stream! {
        loop {
            match rx.recv().await {
                Ok(i) => {
                    let event = Event::default().data(serde_json::to_string(&i).unwrap());

                    yield event;
                }

                Err(e) => {
                    tracing::error!(error = ?e, "Failed to get");
                }
            }
        }
    })
    .keep_alive(KeepAlive::default())
}

async fn check_auth<B>(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    request: Request<B>,
    next: Next<B>,
) -> Response {
    let token = decode_auth_token(auth.token());

    let db = match request.extensions().get::<Db>() {
        Some(s) => s,
        None => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    };

    if let Ok(claims) = token {
        if let Ok(Some(_user)) = get_user_by_username(&db.0, &claims.sub.into()).await {
            let response = next.run(request).await;
            return response;
        }
    }

    (StatusCode::UNAUTHORIZED).into_response()
}

#[derive(Serialize, Deserialize, Clone)]
struct Username(String);

impl Username {
    pub fn inner(&self) -> String {
        self.0.to_owned()
    }
}

impl From<String> for Username {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for Username {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct InviteCode(String);

impl InviteCode {
    pub fn new(username: &str) -> Self {
        Self(format!(
            "{}{}",
            &username[..=2],
            Self::generate_invite_code_digit()
        ))
    }

    pub fn inner(&self) -> String {
        self.0.to_owned()
    }

    fn generate_invite_code_digit() -> String {
        let mut rng = rand::thread_rng();
        let uni_sample = Uniform::from(1001..=9999);
        let code = uni_sample.sample(&mut rng);
        code.to_string()
    }
}

impl From<String> for InviteCode {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct User {
    username: Username,
    invite_code: InviteCode,
    referred_by: Option<Username>,
}

#[derive(Serialize, Deserialize)]
struct DbUser {
    uid: Uuid,
    username: String,
    invite_code: String,
    referred_by: Option<String>,
}

impl From<DbUser> for User {
    fn from(value: DbUser) -> Self {
        Self {
            username: value.username.into(),
            invite_code: value.invite_code.into(),
            referred_by: value.referred_by.map(|r| Username::from(r)),
        }
    }
}

async fn get_user_by_username(
    pool: &PgPool,
    username: &Username,
) -> Result<Option<User>, DatabaseError> {
    let user = sqlx::query_as!(
        DbUser,
        "select * from users where username = $1",
        username.inner()
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| DatabaseError::ServerError)?;

    Ok(user.map(|u| u.into()))
}

async fn get_user_by_invite_code(
    pool: &PgPool,
    invite_code: &InviteCode,
) -> Result<Option<User>, DatabaseError> {
    let user = sqlx::query_as!(
        DbUser,
        "select * from users where invite_code = $1",
        invite_code.inner()
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| DatabaseError::ServerError)?;

    Ok(user.map(|u| u.into()))
}

async fn create_new_user(
    pool: &PgPool,
    username: &Username,
    invite_code: &InviteCode,
    referred_by: Option<Username>,
) -> Result<User, DatabaseError> {
    let user = sqlx::query_as!(
        DbUser,
        "insert into users (uid, username, invite_code, referred_by) values ($1, $2, $3, $4) returning *",
        Uuid::new_v4(),
        username.inner(),
        invite_code.inner(),
        referred_by.map(|r| r.inner())
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        println!("{}", e);
        DatabaseError::ServerError
    })?;

    Ok(user.into())
}

async fn get_user_referral_count(pool: &PgPool, username: &Username) -> Result<u32, DatabaseError> {
    let count = sqlx::query!(
        "select count(*) as referral_count from users where referred_by = $1",
        username.inner(),
    )
    .fetch_one(pool)
    .await
    .map_err(|_| DatabaseError::ServerError)?;

    Ok(count.referral_count.unwrap_or(0) as u32)
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    exp: usize,
}

#[derive(Debug)]
enum JWTError {
    GenerationFailed(jsonwebtoken::errors::ErrorKind),
    DecodeFailed(jsonwebtoken::errors::ErrorKind),
}

impl From<JWTError> for ApiError {
    fn from(_value: JWTError) -> Self {
        Self::AuthenticationError
    }
}

fn generate_auth_token(username: &Username) -> Result<String, JWTError> {
    let exp = SystemTime::now() + Duration::from_secs(86400);
    let claims = Claims {
        iss: "killpowa".to_string(),
        sub: username.inner(),
        exp: exp.duration_since(UNIX_EPOCH).unwrap().as_secs() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret("secret".as_ref()),
    )
    .map_err(|e| JWTError::GenerationFailed(e.into_kind()))?;

    Ok(token)
}

fn decode_auth_token(token: &str) -> Result<Claims, JWTError> {
    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret("secret".as_ref()),
        &Validation::default(),
    )
    .map_err(|e| JWTError::DecodeFailed(e.into_kind()))?;

    Ok(token_data.claims)
}
