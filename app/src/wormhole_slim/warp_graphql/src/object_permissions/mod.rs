use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnerType {
    #[default]
    User,
    Team,
}

impl fmt::Display for OwnerType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "User"),
            Self::Team => write!(f, "Team"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessLevel {
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct Owner;
