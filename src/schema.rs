use std::fmt::Display;
use diesel_derive_enum::DbEnum;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone, Debug, DbEnum)]
pub enum Alliance {
    Red,
    Blue,
}

impl Display for Alliance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, DbEnum)]
pub enum LeftTarmac {
    Yes,
    No,
}

impl Display for LeftTarmac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, DbEnum)]
pub enum Climb {
    No,
    Failed,
    Low,
    Mid,
    High,
    Traversal,
}

impl Display for Climb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

table! {
    use diesel::sql_types::{Integer, Text};
    use super::{AllianceMapping, LeftTarmacMapping, ClimbMapping};
    data (team, match_number) {
        team -> Integer,
        match_number -> Integer,
        alliance -> AllianceMapping,
        left_tarmac -> LeftTarmacMapping,
        auto_high_made -> Integer,
        auto_high_missed -> Integer,
        auto_low_made -> Integer,
        auto_low_missed -> Integer,
        teleop_high_made -> Integer,
        teleop_high_missed -> Integer,
        teleop_low_made -> Integer,
        teleop_low_missed -> Integer,
        climb -> ClimbMapping,
        notes -> Text,
    }
}
