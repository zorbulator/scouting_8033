use std::{ops::{Add, AddAssign}, collections::BTreeMap};
use diesel::prelude::*;
use actix_web::{HttpResponse, error::ErrorInternalServerError, web};
use crate::{HttpResult, models::RobotMatchInfo, DbPool, DatabaseError, schema::Climb};

#[derive(Clone, Default)]
struct ClimbInfo {
    no_attempts: u32,
    fails: u32,
    low_climbs: u32,
    mid_climbs: u32,
    high_climbs: u32,
    traverse_climbs: u32,
}

impl From<RobotMatchInfo> for ClimbInfo {
    fn from(i: RobotMatchInfo) -> Self {
        ClimbInfo {
            no_attempts: matches!(i.climb, Climb::No) as u32,
            fails: matches!(i.climb, Climb::Failed) as u32,
            low_climbs: matches!(i.climb, Climb::Low) as u32,
            mid_climbs: matches!(i.climb, Climb::Mid) as u32,
            high_climbs: matches!(i.climb, Climb::High) as u32,
            traverse_climbs: matches!(i.climb, Climb::Traversal) as u32,
        }
    }
}

impl AddAssign for ClimbInfo {
    fn add_assign(&mut self, rhs: Self) {
        self.no_attempts += rhs.no_attempts;
        self.fails += rhs.fails;
        self.low_climbs += rhs.low_climbs;
        self.mid_climbs += rhs.mid_climbs;
        self.high_climbs += rhs.high_climbs;
        self.traverse_climbs += rhs.traverse_climbs;
    }
}

impl ClimbInfo {
    fn total(&self) -> u32 {
        self.no_attempts + self.fails + self.low_climbs + self.mid_climbs + self.high_climbs + self.traverse_climbs
    }

    fn mean_points(&self) -> f32 {
        (
            self.low_climbs as f32 * 4f32 +
            self.mid_climbs as f32 * 6f32 +
            self.high_climbs as f32 * 10f32 +
            self.traverse_climbs as f32 * 15f32
        )
            / (self.total() as f32)
    }
}

pub async fn get_climb_chart(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
    // could write more of this as an sql query
    let infos: Vec<RobotMatchInfo> = web::block(move || -> Result<Vec<RobotMatchInfo>, DatabaseError> {
        use crate::schema::data::dsl::*;
        let conn = pool.get()?;
        Ok(data.order_by(team.asc()).load(&conn)?)
    }).await.map_err(ErrorInternalServerError)?.map_err(ErrorInternalServerError)?;


    use charts::{Chart, VerticalBarView, ScaleBand, ScaleLinear, BarLabelPosition};
    use std::cmp::Ordering;

    let mut data: BTreeMap<i32, ClimbInfo> = BTreeMap::new();

    for info in infos {
        *data.entry(info.team)
            .or_insert(Default::default()) += info.into();
    }

    // Define chart related sizes.
    let width = 1300;
    let height = 600;
    let (top, right, bottom, left) = (90, 40, 50, 60);

    // Create a linear scale that will interpolate values in [0, 100] range to corresponding
    // values in [0, availableWidth] range (the width of the chart without the margins).
    let y = ScaleLinear::new()
        .set_domain(vec![0_f32, 1_f32])
        .set_range(vec![height - top - bottom, 0]);

    // have to convert to a vec to sort by points
    let mut data: Vec<(i32, ClimbInfo)> = data.into_iter().collect();
    data.sort_by(|(_, a), (_, b)| a.mean_points().partial_cmp(&b.mean_points()).unwrap_or(Ordering::Equal));

    // Create a band scale that maps team numbers to values in the [0, availableHeight]
    // range (the height of the chart without the margins).
    let x = ScaleBand::new()
        .set_domain(
            data.iter().map(|(num, _)| format!("{}", num)).collect())
        .set_range(vec![0, height - top - bottom]);


    let bar_data: Vec<(String, f32, String)> = data.into_iter()
        .flat_map(|(team, info)| {
            [
                (format!("{}", team), (info.fails as f32) / (info.total() as f32), "Failed Climb".to_string()),
                (format!("{}", team), (info.low_climbs as f32) / (info.total() as f32), "Low".to_string()),
                (format!("{}", team), (info.mid_climbs as f32) / (info.total() as f32), "Mid".to_string()),
                (format!("{}", team), (info.high_climbs as f32) / (info.total() as f32), "High".to_string()),
                (format!("{}", team), (info.traverse_climbs as f32) / (info.total() as f32), "Traverse".to_string()),
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
        .add_title(String::from("Climb"))
        .add_legend_at(charts::AxisPosition::Top)
        .add_view(&view)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label("Proportion")
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
