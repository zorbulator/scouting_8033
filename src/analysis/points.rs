use std::{ops::{Add, AddAssign}, collections::BTreeMap};
use diesel::prelude::*;
use actix_web::{HttpResponse, error::ErrorInternalServerError, web};
use crate::{HttpResult, models::RobotMatchInfo, DbPool, DatabaseError};

#[derive(Clone, Default)]
struct TeamPointsInfo {
    num_matches: u32,
    auto_points: i32,
    tele_points: i32,
    climb_points: i32,
}

impl From<RobotMatchInfo> for TeamPointsInfo {
    fn from(i: RobotMatchInfo) -> Self {
        use crate::schema::*;
        let tarmac_points = match i.left_tarmac { LeftTarmac::No => 0, LeftTarmac::Yes => 2 };
        let auto_points = i.auto_high_made * 4 + i.auto_low_made * 2 + tarmac_points;

        let tele_points = i.teleop_high_made * 2 + i.teleop_low_made;

        let climb_points = match i.climb {
            Climb::Low => 4,
            Climb::Mid => 6,
            Climb::High => 10,
            Climb::Traversal => 15,
            _ => 0,
        };

        TeamPointsInfo {
            num_matches: 1,
            auto_points,
            tele_points,
            climb_points,
        }
    }
}

impl Add for TeamPointsInfo {
    type Output = TeamPointsInfo;
    fn add(self, rhs: Self) -> Self::Output {
        TeamPointsInfo {
            num_matches: self.num_matches + rhs.num_matches,
            auto_points: self.auto_points + rhs.auto_points,
            tele_points: self.tele_points + rhs.tele_points,
            climb_points: self.climb_points + rhs.climb_points,
        }
    }
}

impl AddAssign for TeamPointsInfo {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.clone() + rhs;
    }
}

impl TeamPointsInfo {
    fn total(&self) -> i32 {
        self.auto_points + self.tele_points + self.climb_points
    }

    fn mean_total(&self) -> f32 {
        (self.total() as f32) / (self.num_matches as f32)
    }

    fn mean_auto(&self) -> f32 {
        (self.auto_points as f32) / (self.num_matches as f32)
    }

    fn mean_tele(&self) -> f32 {
        (self.auto_points as f32) / (self.num_matches as f32)
    }

    fn mean_climb(&self) -> f32 {
        (self.climb_points as f32) / (self.num_matches as f32)
    }
}

pub async fn get_points_chart(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
    // could write more of this as an sql query
    let infos: Vec<RobotMatchInfo> = web::block(move || -> Result<Vec<RobotMatchInfo>, DatabaseError> {
        use crate::schema::data::dsl::*;
        let conn = pool.get()?;
        Ok(data.order_by(team.asc()).load(&conn)?)
    }).await.map_err(ErrorInternalServerError)?.map_err(ErrorInternalServerError)?;


    use charts::{Chart, VerticalBarView, ScaleBand, ScaleLinear, BarLabelPosition};
    use std::cmp::Ordering;

    let mut data: BTreeMap<i32, TeamPointsInfo> = BTreeMap::new();

    for info in infos {
        *data.entry(info.team)
            .or_insert(Default::default()) += info.into();
    }


    let max_points = data.values()
        .map(|i| i.mean_total())
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal))
        .unwrap_or(200f32);

    // Define chart related sizes.
    let width = 1300;
    let height = 600;
    let (top, right, bottom, left) = (90, 40, 50, 60);

    // Create a linear scale that will interpolate values in [0, 100] range to corresponding
    // values in [0, availableWidth] range (the width of the chart without the margins).
    let y = ScaleLinear::new()
        .set_domain(vec![0_f32, max_points])
        .set_range(vec![height - top - bottom, 0]);

    // have to convert to a vec to sort by points
    let mut data: Vec<(i32, TeamPointsInfo)> = data.into_iter().collect();
    data.sort_by(|(_, a), (_, b)| a.mean_total().partial_cmp(&b.mean_total()).unwrap_or(Ordering::Equal));

    // Create a band scale that maps team numbers to values in the [0, availableHeight]
    // range (the height of the chart without the margins).
    let x = ScaleBand::new()
        .set_domain(
            data.iter().map(|(num, _)| format!("{}", num)).collect())
        .set_range(vec![0, height - top - bottom]);


    let bar_data: Vec<(String, f32, String)> = data.into_iter()
        .flat_map(|(team, info)| {
            [
                (format!("{}", team), (info.auto_points as f32) / (info.num_matches as f32), "Auto".to_string()),
                (format!("{}", team), (info.tele_points as f32) / (info.num_matches as f32), "TeleOp".to_string()),
                (format!("{}", team), (info.climb_points as f32) / (info.num_matches as f32), "Climb".to_string())
            ]
        })
        .collect();

    // Create VerticalBar view that is going to represent the data as vertical bars.
    let view = VerticalBarView::new()
        .set_x_scale(&x)
        .set_y_scale(&y)
        .set_label_position(BarLabelPosition::Center)
        .load_data(&bar_data).unwrap();

    // Generate and save the chart.
    let svg_content = Chart::new()
        .set_width(width)
        .set_height(height)
        .set_margins(top, right, bottom, left)
        .add_title(String::from("Average Points"))
        .add_legend_at(charts::AxisPosition::Top)
        .add_view(&view)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label("Points")
        .add_bottom_axis_label("Team")
        .to_svg().unwrap();

    let document = svg::Document::new()
        .set("width", width)
        .set("height", height)
        .set("viewBox", (0i32, 0i32, width, height))
        .add(svg_content);

    Ok(HttpResponse::Ok().content_type("image/svg+xml; charset=utf-8")
            .body(document.to_string()))
}
