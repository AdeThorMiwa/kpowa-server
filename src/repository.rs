use crate::domain::{
    errors::DatabaseError,
    fields::{InviteCode, User, Username},
    model::DbUser,
};
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

pub struct FetchUserQuery {
    pub username: Option<String>,
    pub auth_user: String,
    pub skip: i64,
    pub limit: i64,
}

pub async fn get_user_by_username(
    pool: &PgPool,
    username: &Username,
) -> Result<Option<User>, DatabaseError> {
    let user = sqlx::query_as!(
        DbUser,
        "select a.*, (select count(referred_by) from users as b where b.referred_by=a.username) as referrals from users as a where username = $1",
        username.inner()
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
         tracing::error!("get user by username failed >>> {}",e);
         DatabaseError::ServerError
    })?;

    Ok(user.map(|u| u.into()))
}

pub async fn get_user_by_invite_code(
    pool: &PgPool,
    invite_code: &InviteCode,
) -> Result<Option<User>, DatabaseError> {
    let user = sqlx::query_as!(
        DbUser,
        "select a.*, (select count(referred_by) from users as b where b.referred_by=a.username) as referrals from users as a where invite_code = $1",
        invite_code.inner()
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("get user by invite failed >>> {}",e);
        DatabaseError::ServerError
    })?;

    Ok(user.map(|u| u.into()))
}

pub async fn create_new_user(
    pool: &PgPool,
    username: &Username,
    invite_code: &InviteCode,
    referred_by: Option<Username>,
) -> Result<(), DatabaseError> {
    sqlx::query!(
        "insert into users (uid, username, invite_code, referred_by) values ($1, $2, $3, $4)",
        Uuid::new_v4(),
        username.inner(),
        invite_code.inner(),
        referred_by.map(|r| r.inner())
    )
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("creating user failed >>> {}", e);
        DatabaseError::ServerError
    })?;

    Ok(())
}

pub async fn fetch_users(
    pool: &PgPool,
    query: FetchUserQuery,
) -> Result<(Vec<User>, i64), DatabaseError> {
    tracing::info!("limit >>> {} offset >>> {}", query.limit, query.skip);
    let mut select_query = QueryBuilder::new("select a.*, (select count(referred_by) from users as b where b.referred_by=a.username) as referrals from users as a ");
    let builder = append_search_param_to_query(&mut select_query, &query, false, false);

    let mut count_query = QueryBuilder::new("select count(*) from users as count ");
    let count_builder = append_search_param_to_query(&mut count_query, &query, true, true);

    let users = builder
        .build_query_as::<DbUser>()
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("getting list of user failed >>> {}", e);
            DatabaseError::ServerError
        })?;

    let count = count_builder.build().fetch_one(pool).await.map_err(|e| {
        tracing::error!("fetch total user count failed >>> {}", e);
        DatabaseError::ServerError
    })?;

    let users: Vec<User> = users.into_iter().map(|u| u.into()).collect();
    Ok((users, count.get("count")))
}

fn append_search_param_to_query<'a>(
    builder: &'a mut QueryBuilder<'a, Postgres>,
    query: &FetchUserQuery,
    skip_ordering: bool,
    skip_pagination: bool,
) -> &'a mut QueryBuilder<'a, Postgres> {
    builder.push(" where username != ");
    builder.push_bind(query.auth_user.clone());

    if let Some(username) = &query.username {
        builder.push(format!(" and username like '%{}%' ", username));
    }

    if !skip_ordering {
        builder.push(" order by created_on desc ");
    }

    if !skip_pagination {
        builder.push(" limit ");
        builder.push_bind(query.limit);

        builder.push(" offset ");
        builder.push_bind(query.skip);
    }

    builder
}
