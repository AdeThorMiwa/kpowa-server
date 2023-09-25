use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use secrecy::ExposeSecret;

use crate::{
    config::JwtConfig,
    domain::{
        errors::JWTError,
        fields::{Claims, Username},
    },
};

pub fn generate_auth_token(
    username: &Username,
    jwt_config: &JwtConfig,
) -> Result<String, JWTError> {
    let exp = SystemTime::now() + Duration::from_secs(jwt_config.exp);
    let claims = Claims {
        iss: jwt_config.iss.clone(),
        sub: username.inner(),
        exp: exp.duration_since(UNIX_EPOCH).unwrap().as_secs() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_config.secret.expose_secret().as_ref()),
    )
    .map_err(|e| {
        tracing::error!("auth token generation failed >>> {}", e);
        JWTError::GenerationFailed(e.into_kind())
    })?;

    Ok(token)
}

pub fn decode_auth_token(token: &str, jwt_config: &JwtConfig) -> Result<Claims, JWTError> {
    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(jwt_config.secret.expose_secret().as_ref()),
        &Validation::default(),
    )
    .map_err(|e| {
        tracing::error!("auth token decode failed >>> {}", e);
        JWTError::DecodeFailed(e.into_kind())
    })?;

    Ok(token_data.claims)
}
