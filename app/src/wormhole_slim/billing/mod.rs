//! Billing stubs for `wormhole-slim` embed builds (no Warp SaaS billing).

use warpui::{AppContext, SingletonEntity};

pub struct BillingManager;

impl BillingManager {
    pub fn new(_ctx: &mut AppContext) -> Self {
        Self
    }
}

impl SingletonEntity for BillingManager {}
