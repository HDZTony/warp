#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RequestLimitRefreshDuration {
    #[default]
    Monthly,
    Weekly,
    EveryTwoWeeks,
}

#[derive(Clone, Debug, Default)]
pub struct RequestLimitInfo {
    pub is_unlimited: bool,
    pub request_limit: i32,
    pub requests_used_since_last_refresh: i32,
    pub next_refresh_time: Option<crate::scalars::Time>,
    pub request_limit_refresh_duration: RequestLimitRefreshDuration,
    pub is_unlimited_voice: bool,
    pub voice_request_limit: i32,
    pub voice_requests_used_since_last_refresh: i32,
    pub is_unlimited_codebase_indices: bool,
    pub max_codebase_indices: i32,
    pub max_files_per_repo: i32,
    pub embedding_generation_batch_size: i32,
}
