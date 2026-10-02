//! Auth extractor: turns `Authorization: Bearer <jwt>` into a `CurrentUser`.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use crate::api::state::SharedState;
use crate::auth::Claims;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: Uuid,
    pub company_id: Uuid,
    pub role: String,
}

impl From<Claims> for CurrentUser {
    fn from(c: Claims) -> Self {
        Self {
            user_id: c.sub,
            company_id: c.company_id,
            role: c.role,
        }
    }
}

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        self.role == "owner" || self.role == "admin"
    }
}

#[async_trait::async_trait]
impl FromRequestParts<SharedState> for CurrentUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> std::result::Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Error::Unauthorized("missing Authorization header".into()))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::Unauthorized("expected Bearer token".into()))?;
        let claims = state.jwt.verify(token)?;
        Ok(claims.into())
    }
}

/// Extractor that additionally requires an admin role.
#[derive(Debug, Clone)]
pub struct AdminUser(pub CurrentUser);

#[async_trait::async_trait]
impl FromRequestParts<SharedState> for AdminUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> std::result::Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if !user.is_admin() {
            return Err(Error::Forbidden("admin role required".into()));
        }
        Ok(AdminUser(user))
    }
}

/// Resolve the caller's company and return its tenant pool. Company isolation:
/// the pool comes from the verified JWT, never from a URL parameter.
pub async fn company_pool(
    state: &SharedState,
    user: &CurrentUser,
    company_id: Option<Uuid>,
) -> Result<(Uuid, std::sync::Arc<sqlx::PgPool>)> {
    let requested = match company_id {
        Some(id) => id,
        None => user.company_id,
    };
    if requested != user.company_id && !user.is_admin() {
        return Err(Error::Forbidden("not a member of this company".into()));
    }
    let row: Option<(String,)> =
        sqlx::query_as("SELECT database_name FROM company WHERE id = $1 AND database_name IS NOT NULL")
            .bind(requested)
            .fetch_optional(&state.pool)
            .await?;
    let Some((database_name,)) = row else {
        return Err(Error::NotFound(format!("company {requested}")));
    };
    let pool = state.tenant_pool(&database_name).await?;
    Ok((requested, pool))
}
