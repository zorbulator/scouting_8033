use std::{ops::AddAssign, collections::BTreeMap};
use actix_web::{HttpResponse, error::ErrorInternalServerError, web};
use crate::{models::RobotMatchInfo, DbPool, HttpResult, DatabaseError};
use diesel::prelude::*;

// only for teleop currently
#[derive(Default)]
struct AccuracyInfo {
    points: i32,
    missed_points: i32,
}

impl AccuracyInfo {
    fn accuracy(&self) -> f32 {
        (self.points as f32) / ((self.points + self.missed_points) as f32)
    }
}

impl AddAssign for AccuracyInfo {
    fn add_assign(&mut self, rhs: Self) {
        self.points += rhs.points;
        self.missed_points += rhs.missed_points;
    }
}

impl From<RobotMatchInfo> for AccuracyInfo {
    fn from(i: RobotMatchInfo) -> Self {
        AccuracyInfo {
            points: i.teleop_high_made * 2 + i.teleop_low_made,
            missed_points: i.teleop_high_missed * 2 + i.teleop_low_missed,
        }
    }
}

pub async fn get_accuracy_chart(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
    // could write more of this as an sql query
    let infos: Vec<RobotMatchInfo> = web::block(move || -> Result<Vec<RobotMatchInfo>, DatabaseError> {
        use crate::schema::data::dsl::*;
        let conn = pool.get()?;
        Ok(data.order_by(team.asc()).load(&conn)?)
    }).await.map_err(ErrorInternalServerError)?.map_err(ErrorInternalServerError)?;


    use charts::{Chart, VerticalBarView, ScaleBand, ScaleLinear, BarLabelPosition};
    use std::cmp::Ordering;

    let mut data: BTreeMap<i32, AccuracyInfo> = BTreeMap::new();

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
    let mut data: Vec<(i32, AccuracyInfo)> = data.into_iter().collect();
    data.sort_by(|(_, a), (_, b)| a.accuracy().partial_cmp(&b.accuracy()).unwrap_or(Ordering::Equal));

    // Create a band scale that maps team numbers to values in the [0, availableHeight]
    // range (the height of the chart without the margins).
    let x = ScaleBand::new()
        .set_domain(
            data.iter().map(|(num, _)| format!("{}", num)).collect())
        .set_range(vec![0, height - top - bottom]);


    let bar_data: Vec<(String, f32)> = data.into_iter()
        .map(|(team, info)| {
            (format!("{}", team), info.accuracy())
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
        .add_title(String::from("Teleop Accuracy"))
        .add_legend_at(charts::AxisPosition::Top)
        .add_view(&view)
        .add_axis_bottom(&x)
        .add_axis_left(&y)
        .add_left_axis_label("Accuracy Ratio")
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

