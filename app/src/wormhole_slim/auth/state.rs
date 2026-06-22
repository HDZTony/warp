//! Minimal auth state for `wormhole-slim` embed builds.

use std::sync::Arc;

use warpui::{Entity, ModelContext, SingletonEntity};

/// Minimal logged-in/out state for slim builds.
#[derive(Debug, Default, Clone)]
pub struct AuthState {
    is_onboarded: bool,
}

impl AuthState {
    pub fn is_logged_in(&self) -> bool {
        false
    }

    pub fn is_anonymous_or_logged_out(&self) -> bool {
        true
    }

    pub fn is_user_anonymous(&self) -> Option<bool> {
        Some(true)
    }

    pub fn user_id(&self) -> Option<crate::auth::UserUid> {
        None
    }

    pub fn set_is_onboarded(&mut self, is_onboarded: bool) {
        self.is_onboarded = is_onboarded;
    }

    pub fn is_onboarded(&self) -> bool {
        self.is_onboarded
    }
}

pub enum AuthStateProviderEvent {}

pub struct AuthStateProvider {
    auth_state: Arc<AuthState>,
}

impl AuthStateProvider {
    pub fn new(auth_state: Arc<AuthState>) -> Self {
        Self { auth_state }
    }

    pub fn get(&self) -> Arc<AuthState> {
        self.auth_state.clone()
    }
}

impl Entity for AuthStateProvider {
    type Event = AuthStateProviderEvent;
}

impl SingletonEntity for AuthStateProvider {}

#[cfg(test)]
impl AuthStateProvider {
    pub fn mock(_ctx: &mut ModelContext<Self>) -> Self {
        Self::new(Arc::new(AuthState::default()))
    }
}
