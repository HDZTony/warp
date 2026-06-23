use std::borrow::Cow;

#[derive(Clone, Debug, Default)]
pub struct Id(pub String);

impl Id {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct RequestOptions {
    pub path_prefix: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GraphQLError {
    #[error("error sending request")]
    RequestError(String),
    #[error("not authorized for staging")]
    StagingAccessBlocked,
    #[error("blocked by IAP challenge")]
    IapChallengeBlocked,
    #[error("received non-OK response code {status}")]
    HttpError { status: u16, body: String },
    #[error("Failed to deserialize GraphQL response: {0}")]
    ResponseError(String),
}

pub struct OperationMarker<R> {
    _response: std::marker::PhantomData<R>,
}

impl<R> OperationMarker<R> {
    pub fn new() -> Self {
        Self {
            _response: std::marker::PhantomData,
        }
    }
}

impl<R> Default for OperationMarker<R> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Operation<R>: Send + Sync {}

impl<R, T: Send + Sync> Operation<R> for T {}

pub fn get_user_facing_error_message(error: &crate::error::UserFacingErrorInterface) -> String {
    let _ = error;
    String::new()
}

pub trait UserFacingError {}

impl UserFacingError for crate::error::UserFacingErrorInterface {}

pub fn get_request_context() -> RequestContext {
    RequestContext
}

#[derive(Clone, Debug, Default)]
pub struct RequestContext;

pub mod get_request_context {
    pub use super::get_request_context;
}
