use crate::scalars::Time;

#[derive(Clone, Debug, Default)]
pub struct ScheduledAgentHistory {
    pub last_ran: Option<Time>,
    pub next_run: Option<Time>,
}
