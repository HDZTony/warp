#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BonusGrantType {
    #[default]
    Any,
    AmbientOnly,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AddonCreditAutoReloadStatus {
    #[default]
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripeSubscriptionPlan {
    Unknown,
    Build,
    BuildMax,
    BuildBusiness,
    Turbo,
    Business,
    Lightspeed,
    Pro,
    Team,
    Enterprise,
}

impl Default for StripeSubscriptionPlan {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Debug, Default)]
pub struct AddonCreditsOption {
    pub credits: i32,
    pub price_usd_cents: i32,
}

#[derive(Clone, Debug, Default)]
pub struct OveragesPricing {
    pub price_per_request_usd_cents: i32,
}

#[derive(Clone, Debug, Default)]
pub struct PlanPricing {
    pub plan: StripeSubscriptionPlan,
    pub monthly_plan_price_per_month_usd_cents: i32,
    pub yearly_plan_price_per_month_usd_cents: i32,
    pub request_limit: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct PricingInfo {
    pub overages: OveragesPricing,
    pub plans: Vec<PlanPricing>,
    pub addon_credits_options: Vec<AddonCreditsOption>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ServiceAgreementType {
    #[default]
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct ServiceAgreement;
