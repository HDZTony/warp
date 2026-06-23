#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpaceType {
    #[default]
    User,
    Team,
}

#[derive(Clone, Debug, Default)]
pub struct Space {
    pub space_type: SpaceType,
}
