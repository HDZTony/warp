#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Experiment {
    SessionSharingExperiment,
    SessionSharingControl,
    BuildPlanAutoReloadControl,
    BuildPlanAutoReloadBannerToggle,
    BuildPlanAutoReloadPostPurchaseModal,
    DisableAgentModeExperiment,
    EnvVarsEarlyAccessExperiment,
    AgentModeAnalyticsExperiment,
    TmuxSshWarpificationControl,
    TmuxSshWarpificationExperiment,
    WindowsLaunchExperiment,
    CodebaseContextControl,
    CodebaseContextExperiment,
    SuggestedCodeDiffsControl,
    SuggestedCodeDiffsExperiment,
    PromptSuggestionsViaMaaControl,
    PromptSuggestionsViaMaaOob,
    FreeUserNoAiControl,
    FreeUserNoAiExperiment,
    OzMultiHarnessControl,
    OzMultiHarnessExperiment,
}

#[derive(Clone, Debug, Default)]
pub struct ServerExperiment;
