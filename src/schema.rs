use diesel_derive_enum::DbEnum;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Clone, Debug, DbEnum)]
pub enum Alliance {
    Red,
    Blue,
}

#[derive(Serialize, Deserialize, Clone, Debug, DbEnum)]
pub enum LeftTarmac {
    Yes,
    No,
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


table! {
    use diesel::sql_types::Integer;
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
    }
}
