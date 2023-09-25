use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};

use crate::domain::{
    errors::JWTError,
    fields::{Claims, Username},
};

pub fn generate_auth_token(username: &Username) -> Result<String, JWTError> {
    let exp = SystemTime::now() + Duration::from_secs(864000);
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
    .map_err(|e| {
        tracing::error!("auth token generation failed >>> {}", e);
        JWTError::GenerationFailed(e.into_kind())
    })?;

    Ok(token)
}

pub fn decode_auth_token(token: &str) -> Result<Claims, JWTError> {
    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret("secret".as_ref()),
        &Validation::default(),
    )
    .map_err(|e| {
        tracing::error!("auth token decode failed >>> {}", e);
        JWTError::DecodeFailed(e.into_kind())
    })?;

    Ok(token_data.claims)
}
