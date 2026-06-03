//! Request extraction + auth/validation guards — an "axum-lite" layer for the
//! workers-rs router.
//!
//! The problem this solves: every workspace-scoped handler used to repeat ~25
//! lines of `verify_session` / `resolve_workspace` / membership / body-parse /
//! validate, each branch hand-rolling a CORS error response because
//! `worker::Error` carries no `(status, code)`.
//!
//! The shape, borrowed from axum at minimal dosage:
//!   * [`ApiError`] — one rejection type that knows its HTTP status + i18n code
//!     and converts to a CORS response in one place.
//!   * [`FromParts`] — "extract this from the request without touching the
//!     body". Implemented once per access level ([`Session`], [`Member`],
//!     [`Admin`]). Adding a level later is one more impl.
//!   * [`route`] / [`route_json`] — the only glue. They run the extraction,
//!     then call a handler that receives already-validated values. `route_json`
//!     is the single thing that consumes the body, and it does so last.
//!
//! Registration reads the access policy off the route table:
//! ```ignore
//! .get_async ("/api/w/:slug/todos", route     (todos::list_todos))   // Member
//! .post_async("/api/w/:slug/todos", route_json(todos::create_todo))  // Member + DTO
//! ```
//! `P` and the body type `B` are inferred from the handler's signature, so no
//! turbofish is needed at the call site.

use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use worker::*;

use crate::db;
use crate::middleware::{self, AuthError, Claims};

/// A handler rejection: an HTTP status plus a stable i18n code. The code
/// doubles as the response `detail`, so no English prose lands in source — the
/// SPA renders the code via `tr()`.
pub struct ApiError {
    pub status: u16,
    pub code: Cow<'static, str>,
}

impl ApiError {
    pub fn new(status: u16, code: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            code: code.into(),
        }
    }
    pub fn bad_request(code: impl Into<Cow<'static, str>>) -> Self {
        Self::new(400, code)
    }
    pub fn forbidden(code: impl Into<Cow<'static, str>>) -> Self {
        Self::new(403, code)
    }
    pub fn not_found(code: impl Into<Cow<'static, str>>) -> Self {
        Self::new(404, code)
    }
    /// Internal faults keep a generic code; the real cause is logged by
    /// `error_with_cors` (5xx branch), not leaked to the client.
    pub fn internal() -> Self {
        Self::new(500, "internal")
    }

    pub fn into_response(self, req: &Request) -> Result<Response> {
        middleware::error_with_cors(req, self.status, &self.code, &self.code)
    }
}

impl From<AuthError> for ApiError {
    fn from(e: AuthError) -> Self {
        Self::new(e.status(), e.code())
    }
}

/// Resolve the `:slug` path param to its workspace metadata row. Single source
/// of truth — replaces the eight verbatim copies that used to live one per
/// route module.
async fn resolve_workspace(ctx: &RouteContext<()>) -> std::result::Result<db::WorkspaceMetaRow, ApiError> {
    let slug = ctx
        .param("slug")
        .ok_or_else(|| ApiError::not_found("workspace.not_found"))?;
    let index_db = db::get_index_db(&ctx.env).map_err(|_| ApiError::internal())?;
    db::lookup_workspace_by_slug(&index_db, slug)
        .await
        .map_err(|_| ApiError::internal())?
        .ok_or_else(|| ApiError::not_found("workspace.not_found"))
}

/// "Extract `Self` from the request parts, without consuming the body."
/// The body-consuming step is handled separately by [`route_json`], matching
/// axum's rule that only the last extractor touches the body.
#[async_trait(?Send)]
pub trait FromParts: Sized {
    async fn from_parts(
        req: &Request,
        ctx: &RouteContext<()>,
    ) -> std::result::Result<Self, ApiError>;
}

/// Authenticated session only — no workspace scoping. For routes like
/// `/api/me` that are user-scoped rather than workspace-scoped.
pub struct Session(pub Claims);

#[async_trait(?Send)]
impl FromParts for Session {
    async fn from_parts(
        req: &Request,
        ctx: &RouteContext<()>,
    ) -> std::result::Result<Self, ApiError> {
        let claims = middleware::verify_session(req, &ctx.env).await?;
        Ok(Session(claims))
    }
}

/// Authenticated session + resolved workspace + confirmed membership. The
/// common case for every `/api/w/:slug/...` route.
pub struct Member {
    pub claims: Claims,
    pub ws: db::WorkspaceMetaRow,
}

#[async_trait(?Send)]
impl FromParts for Member {
    async fn from_parts(
        req: &Request,
        ctx: &RouteContext<()>,
    ) -> std::result::Result<Self, ApiError> {
        let claims = middleware::verify_session(req, &ctx.env).await?;
        let ws = resolve_workspace(ctx).await?;
        if !claims.workspaces.contains(&ws.slug) {
            return Err(ApiError::forbidden("auth.not_member"));
        }
        Ok(Member { claims, ws })
    }
}

/// `Member` plus the `admin` role in that workspace (super-admin overrides).
pub struct Admin {
    pub claims: Claims,
    pub ws: db::WorkspaceMetaRow,
}

#[async_trait(?Send)]
impl FromParts for Admin {
    async fn from_parts(
        req: &Request,
        ctx: &RouteContext<()>,
    ) -> std::result::Result<Self, ApiError> {
        let claims = middleware::verify_session(req, &ctx.env).await?;
        let ws = resolve_workspace(ctx).await?;
        if !claims.workspaces.contains(&ws.slug) {
            return Err(ApiError::forbidden("auth.not_member"));
        }
        let index_db = db::get_index_db(&ctx.env).map_err(|_| ApiError::internal())?;
        let is_admin = middleware::is_workspace_admin_by_slug(&index_db, &claims.sub, &ws.slug)
            .await
            .map_err(|_| ApiError::internal())?;
        if !is_admin && !middleware::is_super_admin(&ctx.env, &claims) {
            return Err(ApiError::forbidden("auth.not_admin"));
        }
        Ok(Admin { claims, ws })
    }
}

/// Pick the first validation error's code as the response code. For a
/// single-field violation this is deterministic; with several invalid fields
/// the choice is arbitrary, which is acceptable — the client fixes one at a
/// time and re-submits.
fn first_validation_code(errs: &validator::ValidationErrors) -> Cow<'static, str> {
    errs.field_errors()
        .values()
        .next()
        .and_then(|v| v.first())
        .map(|e| e.code.clone())
        .unwrap_or(Cow::Borrowed("validation_error"))
}

/// The async future type our combinators hand back to the router. workers-rs
/// re-boxes it, so a plain boxed future here is the simplest nameable return
/// type (nested `impl Fn -> impl Future` is not expressible on stable).
type RouteFuture = Pin<Box<dyn Future<Output = Result<Response>>>>;

/// Guard a handler that needs no body: extract `P`, then call
/// `handler(req, ctx, P)`. On rejection, short-circuit with the CORS error.
pub fn route<P, H, Fut>(handler: H) -> impl Fn(Request, RouteContext<()>) -> RouteFuture
where
    P: FromParts + 'static,
    H: Fn(Request, RouteContext<()>, P) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<Response>> + 'static,
{
    move |req, ctx| {
        let handler = handler.clone();
        Box::pin(async move {
            let p = match P::from_parts(&req, &ctx).await {
                Ok(p) => p,
                Err(e) => return e.into_response(&req),
            };
            handler(req, ctx, p).await
        })
    }
}

/// Guard a handler that needs a validated JSON body: extract `P`, then
/// deserialize the body into `B` (400 on malformed JSON), then `B::validate()`
/// (400 + the first violation's i18n code), then call
/// `handler(req, ctx, P, B)`. The body is consumed here and only here, last.
pub fn route_json<P, B, H, Fut>(handler: H) -> impl Fn(Request, RouteContext<()>) -> RouteFuture
where
    P: FromParts + 'static,
    B: serde::de::DeserializeOwned + validator::Validate + 'static,
    H: Fn(Request, RouteContext<()>, P, B) -> Fut + Clone + 'static,
    Fut: Future<Output = Result<Response>> + 'static,
{
    move |mut req, ctx| {
        let handler = handler.clone();
        Box::pin(async move {
            let p = match P::from_parts(&req, &ctx).await {
                Ok(p) => p,
                Err(e) => return e.into_response(&req),
            };
            let body: B = match req.json().await {
                Ok(b) => b,
                Err(_) => return ApiError::bad_request("bad_request").into_response(&req),
            };
            if let Err(errs) = body.validate() {
                return ApiError::bad_request(first_validation_code(&errs)).into_response(&req);
            }
            handler(req, ctx, p, body).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grumps_core::dto::CreateTodoRequest;
    use validator::Validate;

    // The (status, code) a rejection carries IS the API contract the SPA reads
    // and localises — pin it so a careless edit to AuthError can't silently
    // change a 401 into a 403 or rename an i18n key.
    #[test]
    fn auth_error_maps_to_status_and_code() {
        let e: ApiError = AuthError::Unauthenticated.into();
        assert_eq!(e.status, 401);
        assert_eq!(e.code, "auth.unauthenticated");

        let e: ApiError = AuthError::CsrfMismatch.into();
        assert_eq!(e.status, 403);
        assert_eq!(e.code, "auth.csrf_mismatch");
    }

    #[test]
    fn helper_constructors_set_status() {
        assert_eq!(ApiError::bad_request("x").status, 400);
        assert_eq!(ApiError::forbidden("x").status, 403);
        assert_eq!(ApiError::not_found("x").status, 404);
        assert_eq!(ApiError::internal().status, 500);
    }

    #[test]
    fn first_validation_code_extracts_the_field_code() {
        // An empty title trips `length(min = 1, code = "todo.title_invalid")`.
        let errs = CreateTodoRequest {
            title: String::new(),
            ..Default::default()
        }
        .validate()
        .unwrap_err();
        assert_eq!(first_validation_code(&errs), "todo.title_invalid");
    }
}
