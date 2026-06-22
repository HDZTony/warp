//! Local billing types used by [`super::workspace`] when `wormhole-slim` omits `warp_graphql`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddonCreditAutoReloadStatus {
    Failed,
    Succeeded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceAgreementStatus {
    Active,
    Canceled,
    PastDue,
    Unpaid,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceAgreementType {
    Enterprise,
    Legacy,
    ProTrial,
    Prosumer,
    SelfServe,
    TeamTrial,
    Turbo,
    Business,
    Lightspeed,
    Other(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceAgreement {
    pub addon_credit_auto_reload_status: Option<AddonCreditAutoReloadStatus>,
    pub current_period_end: DateTime<Utc>,
    pub status: ServiceAgreementStatus,
    pub stripe_subscription_id: Option<String>,
    pub type_: ServiceAgreementType,
    pub sunsetted_to_build_ts: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiCreditsUsageAndCostSubjectType {
    Team,
    User,
    ServiceAccount,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiCreditsUsageAndCostType {
    BaseLimit,
    BonusGrant,
    Payg,
    AmbientBonusGrant,
    Aggregate,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiCreditsUsageBucket {
    Ai,
    Compute,
    Platform,
    SuggestedCodeDiffs,
    Voice,
    Aggregate,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiCreditsUsageSource {
    Local,
    Cloud,
    Aggregate,
    Other(String),
}

#[cfg(not(feature = "wormhole-slim"))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddonCreditsOption {
    pub credits: i32,
    pub price_usd_cents: i32,
}

#[cfg(feature = "wormhole-slim")]
pub use warp_graphql::billing::AddonCreditsOption;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BonusGrantType {
    AmbientOnly,
    Any,
}
