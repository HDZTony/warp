//! Disable Warp SaaS surfaces that Wormhole does not productize.
//!
//! Embedded `warp-oss-wormhole` excises (runtime flags + UI wiring):
//! - Warp Drive cloud sync
//! - warp.dev account login / anonymous signup / cloud object polling
//! - Warp Teams
//! - Referral / growth
//! - Warp autoupdate, billing page, remote SSH / HOA

use warp_core::features::FeatureFlag;

const SLIM_DISABLED_FLAGS: &[FeatureFlag] = &[
    FeatureFlag::CloudObjects,
    FeatureFlag::SharedWithMe,
    FeatureFlag::AgentModeWorkflows,
    FeatureFlag::FetchGenericStringObjects,
    FeatureFlag::MultiWorkspace,
    FeatureFlag::CreatingSharedSessions,
    FeatureFlag::ViewingSharedSessions,
    FeatureFlag::WelcomeBlock,
    FeatureFlag::KnowledgeSidebar,
    FeatureFlag::WelcomeTab,
    FeatureFlag::GetStartedTab,
    FeatureFlag::BillingAndUsagePageV2,
    FeatureFlag::RemoteCodeReview,
    FeatureFlag::HandoffLocalCloud,
    FeatureFlag::TeamApiKeys,
    FeatureFlag::NamedAgents,
    FeatureFlag::Changelog,
    FeatureFlag::Autoupdate,
    FeatureFlag::CrashReporting,
    FeatureFlag::CocoaSentry,
    FeatureFlag::SendTelemetryToFile,
    FeatureFlag::SshRemoteServer,
    FeatureFlag::RemoteCodebaseIndexing,
    FeatureFlag::HOARemoteControl,
    FeatureFlag::HOAOnboardingFlow,
    FeatureFlag::HoaCodeReview,
    FeatureFlag::HOANotifications,
];

/// Whether Warp cloud account / Drive / Teams services should be inactive.
pub fn warp_cloud_disabled() -> bool {
    super::is_embedded()
}

/// Turn off Warp SaaS feature flags after the channel defaults are applied.
pub fn apply_slim_feature_flags() {
    if !warp_cloud_disabled() {
        return;
    }
    for flag in SLIM_DISABLED_FLAGS {
        flag.set_enabled(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn slim_profile_active_when_embedded_env_set() {
        let key = super::super::EMBEDDED_ENV;
        let previous = env::var(key).ok();
        env::set_var(key, "1");
        assert!(warp_cloud_disabled());
        match previous {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }
    }
}
