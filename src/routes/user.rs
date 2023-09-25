use std::sync::Arc;

use crate::{
    app::AppState,
    domain::{errors::ApiError, fields::User},
    repository::{fetch_users, FetchUserQuery},
};
use axum::{
    extract::{Query, State},
    Extension, Json,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct AuthenticatedUserResponse {
    #[serde(flatten)]
    user: User,
}

#[derive(Deserialize)]
pub struct QueryParams {
    username: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    has_next: bool,
    has_prev: bool,
    current_page: i64,
    total_pages: i64,
}

#[derive(Serialize)]
pub struct GetUsersResponse {
    users: Vec<User>,
    #[serde(flatten)]
    pagination: Pagination,
}

pub async fn get_authenticated_user(
    Extension(user): Extension<User>,
) -> Result<Json<AuthenticatedUserResponse>, ApiError> {
    Ok(Json(AuthenticatedUserResponse { user }))
}

pub async fn get_users(
    State(state): State<Arc<AppState>>,
    Query(query): Query<QueryParams>,
    Extension(user): Extension<User>,
) -> Result<Json<GetUsersResponse>, ApiError> {
    let pool = state.get_pool();
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(10);
    let skip = (page - 1) * limit;

    let query = FetchUserQuery {
        username: query.username,
        auth_user: user.username.inner(),
        limit,
        skip,
    };

    let (users, count) = fetch_users(&pool, query).await?;

    let total_pages = (count / limit) + 1;
    Ok(Json(GetUsersResponse {
        users,
        pagination: Pagination {
            has_next: page < total_pages,
            has_prev: page > 1,
            current_page: page,
            total_pages,
        },
    }))
}
