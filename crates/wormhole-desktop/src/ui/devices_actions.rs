#[derive(Debug, Clone)]
pub enum DevicesAction {
    Refresh,
    OpenNode(String),
    BackToGrid,
    OpenJoinModal,
    CloseJoinModal,
    PasteJoinInvite,
    SubmitJoin,
    CopyInvite,
    ToggleClusterPicker,
    CloseClusterPicker,
    SelectCluster(String),
    ShareBack,
    ShareForward,
    ShareNavigate {
        volume_id: Option<String>,
        name: String,
    },
    OpenShareAddModal,
    CloseShareAddModal,
    BrowseShareAddPath,
    SubmitShareAdd,
}
