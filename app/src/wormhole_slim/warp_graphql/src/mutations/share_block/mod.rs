#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DisplaySetting {
    Command,
    CommandAndOutput,
    Output,
    Other(String),
}
