//! GraphQL helpers for `wormhole-slim` embed builds.

pub use warp_graphql::client::{
    get_request_context, get_user_facing_error_message, GraphQLError, RequestOptions,
};

pub fn default_request_options() -> RequestOptions {
    RequestOptions {
        #[cfg(feature = "agent_mode_evals")]
        path_prefix: Some("/agent-mode-evals".to_string()),
        ..Default::default()
    }
}

pub mod schema {
    pub mod types {}
}
