//! Minimal [`AuthManager`] stub for `wormhole-slim` embed builds.

use std::sync::Arc;

use warpui::{Entity, ModelContext, SingletonEntity};

use crate::auth::auth_state::{AuthState, AuthStateProvider};
use crate::auth::auth_view_modal::{AuthRedirectPayload, AuthViewVariant};
use crate::server::server_api::auth::UserAuthenticationError;

#[derive(Debug)]
pub enum AuthManagerEvent {
    AuthComplete,
    AuthFailed(UserAuthenticationError),
    CreateAnonymousUserFailed,
    SkippedLogin,
    NeedsReauth,
    AttemptedLoginGatedFeature {
        auth_view_variant: AuthViewVariant,
    },
    LoginOverrideDetected(AuthRedirectPayload),
    MintCustomTokenFailed(crate::server::server_api::auth::MintCustomTokenError),
    ReceivedDeviceAuthorizationCode {
        #[cfg_attr(target_family = "wasm", allow(unused))]
        verification_url: String,
        #[cfg_attr(target_family = "wasm", allow(unused))]
        verification_url_complete: Option<String>,
        #[cfg_attr(target_family = "wasm", allow(unused))]
        user_code: String,
    },
}

pub struct AuthManager {
    #[allow(dead_code)]
    auth_state: Arc<AuthState>,
}

impl AuthManager {
    pub fn new(
        _server_api: Arc<crate::server::server_api::ServerApi>,
        _auth_client: Arc<dyn crate::server::server_api::auth::AuthClient>,
        ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self {
            auth_state: AuthStateProvider::as_ref(ctx).get().clone(),
        }
    }

    #[cfg(test)]
    pub fn new_for_test(ctx: &mut ModelContext<Self>) -> Self {
        Self {
            auth_state: AuthStateProvider::as_ref(ctx).get().clone(),
        }
    }

    pub fn set_user_onboarded(&self, _ctx: &mut ModelContext<Self>) {}

    pub fn initialize_user_from_auth_payload(
        &mut self,
        _auth_payload: AuthRedirectPayload,
        _enforce_state_validation: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn resume_interrupted_auth_payload(
        &mut self,
        _auth_payload: AuthRedirectPayload,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn initialize_user_from_session_cookie(&self, _ctx: &mut ModelContext<Self>) {}

    pub fn refresh_user(&self, _ctx: &mut ModelContext<Self>) {}

    pub fn authorize_device(&self, _ctx: &mut ModelContext<Self>) {}

    pub fn set_needs_reauth(&self, _needs_reauth: bool, _ctx: &mut ModelContext<Self>) {}

    pub fn create_anonymous_user(
        &mut self,
        _entrypoint: Option<crate::server::telemetry::AnonymousUserSignupEntrypoint>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn attempt_login_gated_feature(
        &mut self,
        _feature: &'static str,
        _auth_view_variant: AuthViewVariant,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn anonymous_user_hit_drive_object_limit(&self, _ctx: &mut ModelContext<Self>) {}

    pub fn initiate_anonymous_user_linking(
        &mut self,
        _entrypoint: crate::server::telemetry::AnonymousUserSignupEntrypoint,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn open_url_maybe_with_anonymous_token<F>(
        &self,
        ctx: &mut ModelContext<Self>,
        _url_fn: F,
    ) where
        F: FnOnce(Option<&str>) -> String,
    {
        let _ = ctx;
    }

    pub fn copy_anonymous_user_linking_url_to_clipboard(&self, _ctx: &mut ModelContext<Self>) {}

    pub fn sign_up_url(&mut self) -> String {
        String::new()
    }

    pub fn sign_in_url(&mut self) -> String {
        String::new()
    }

    pub fn upgrade_url(&mut self) -> String {
        String::new()
    }

    pub fn login_options_url(&mut self, _custom_token: &str) -> String {
        String::new()
    }

    pub fn link_sso_url(&mut self, _email: &str) -> String {
        String::new()
    }

    pub fn log_out(&mut self, _ctx: &mut ModelContext<Self>) {}
}

impl Entity for AuthManager {
    type Event = AuthManagerEvent;
}

impl SingletonEntity for AuthManager {}

pub type LoginGatedFeature = &'static str;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PersistedCurrentUserInformation {
    pub email: String,
}
