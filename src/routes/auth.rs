use crate::{
    app::{AppState, Db},
    config::Config,
    domain::{
        errors::ApiError,
        events::{AppEvent, NewReferralEvent},
        fields::{InviteCode, Username},
    },
    repository::{create_new_user, get_user_by_invite_code, get_user_by_username},
    utils::jwt::{decode_auth_token, generate_auth_token},
};
use axum::{
    extract::State,
    headers::{authorization::Bearer, Authorization},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json, TypedHeader,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateRequest {
    username: Username,
    invitation_code: Option<InviteCode>,
}

#[derive(Serialize)]
pub struct AuthenticateResponse {
    token: String,
}

impl From<String> for AuthenticateResponse {
    fn from(token: String) -> Self {
        Self { token }
    }
}

pub async fn authenticate(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AuthenticateRequest>,
) -> Result<Json<AuthenticateResponse>, ApiError> {
    let pool = state.get_pool();
    tracing::info!("authenticating user >>> {}", payload.username);
    let user = get_user_by_username(&pool, &payload.username).await?;

    if let Some(user) = user {
        let _ = state.get_sender().send(AppEvent::NewLogin(user.clone()));
        let token = generate_auth_token(&user.username, &state.config.jwt)?;
        return Ok(Json(token.into()));
    }

    let referrer_username = if let Some(invite_code) = payload.invitation_code {
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

    let _ = create_new_user(&pool, &payload.username, &invite_code, referrer_username).await?;
    let user = get_user_by_username(&pool, &payload.username)
        .await?
        .unwrap();
    if user.referred_by.is_some() {
        let _ = state
            .get_sender()
            .send(AppEvent::NewReferral(NewReferralEvent {
                referred_user: user.clone().username,
                referrer: user.clone().referred_by.unwrap(),
            }));
    }

    let _ = state.get_sender().send(AppEvent::NewRegister(user.clone()));
    let token = generate_auth_token(&user.username, &state.config.jwt)?;
    Ok(Json(token.into()))
}

pub async fn check_auth<B>(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Response {
    let config = match request.extensions().get::<Config>() {
        Some(c) => c,
        None => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    };

    let token = decode_auth_token(auth.token(), &config.jwt);

    let db = match request.extensions().get::<Db>() {
        Some(s) => s,
        None => return (StatusCode::INTERNAL_SERVER_ERROR).into_response(),
    };

    if let Ok(claims) = token {
        if let Ok(Some(user)) = get_user_by_username(&db.inner(), &claims.sub.into()).await {
            request.extensions_mut().insert(user);
            let response = next.run(request).await;
            return response;
        }
    }

    (StatusCode::UNAUTHORIZED).into_response()
}
