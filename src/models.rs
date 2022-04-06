use serde::{Deserialize, Serialize};
use diesel_derives::{Queryable, Insertable};
use crate::schema::*;


#[derive(Serialize, Deserialize, Debug, Clone, Queryable, Insertable)]
#[table_name="data"]
pub struct RobotMatchInfo {
    pub team: i32,
    pub match_number: i32,
    pub alliance: Alliance,
    pub left_tarmac: LeftTarmac,
    pub auto_high_made: i32,
    pub auto_high_missed: i32,
    pub auto_low_made: i32,
    pub auto_low_missed: i32,
    pub teleop_high_made: i32,
    pub teleop_high_missed: i32,
    pub teleop_low_made: i32,
    pub teleop_low_missed: i32,
    pub climb: Climb,
    pub notes: String,
}

