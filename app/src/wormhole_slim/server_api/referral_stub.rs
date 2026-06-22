//! Referrals client stub for `wormhole-slim`.

use anyhow::Result;
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

pub struct ReferralInfo {
    pub url: String,
    pub code: String,
    pub number_claimed: usize,
    pub is_referred: bool,
}

#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait ReferralsClient: 'static + Send + Sync {
    async fn get_referral_info(&self) -> Result<ReferralInfo>;
    async fn send_invite(&self, _emails: Vec<String>) -> Result<Vec<String>>;
}

#[derive(Debug, Default)]
pub struct StubReferralsClient;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ReferralsClient for StubReferralsClient {
    async fn get_referral_info(&self) -> Result<ReferralInfo> {
        Ok(ReferralInfo {
            url: String::new(),
            code: String::new(),
            number_claimed: 0,
            is_referred: false,
        })
    }

    async fn send_invite(&self, _emails: Vec<String>) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}
