// using macro_use syntax because a normal use didn't seem to be doing it
// should probably figure out why and `use` the correct macros
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate migrations_macros;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::ops::{Add, AddAssign};

use actix_web::body::MessageBody;
use actix_web::error::ErrorInternalServerError;
use actix_web::http::header::{TryIntoHeaderValue, HeaderValue, InvalidHeaderValue};
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result as HttpResult, dev::ServiceRequest, error::ErrorForbidden};
use actix_web_httpauth::extractors::AuthenticationError;
use actix_web_httpauth::headers::www_authenticate::Challenge;
use actix_web_httpauth::{middleware::HttpAuthentication, extractors::basic::BasicAuth};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel::r2d2::ConnectionManager;
use diesel_migrations::embed_migrations;
use log::{error, info};
use askama::Template;
use models::RobotMatchInfo;

type DbPool = diesel::r2d2::Pool<ConnectionManager<SqliteConnection>>;

// embed the migrations that create a valid database in the binary
embed_migrations!();

mod models;
mod schema;

#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    #[error("database pool error: {0}")]
    PoolError(#[from] r2d2::Error),
    #[error("database returned error: {0}")]
    DieselError(#[from] diesel::result::Error),
}

#[derive(Template)]
#[template(path = "data-listing.html")]
pub struct DataListing {
    data: Vec<RobotMatchInfo>
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "info");
    pretty_env_logger::init();

    // could get from .env but that extra file isn't really necessary
    let database_url = String::from("data.db");

    let manager = ConnectionManager::<SqliteConnection>::new(database_url.clone());

    let pool = diesel::r2d2::Pool::builder()
        .build(manager)
        .expect("failed to create database connection pool");

    // connect to the database to check it and create it if it doesn't work
    // If there is no database the connection will create one but it won't have the table until this runs
    if let Ok(conn) = pool.get() {
        use crate::schema::data::dsl::*;
        if let Err(_) = data.load::<RobotMatchInfo>(&conn) {
            info!("Unable to get data; database either doesn't exist or is empty or corrupted");
            info!("Running migrations to create a valid database...");
            embedded_migrations::run(&conn).expect("unable to run migrations to create database");
        } else {
            info!("Using existing database");
        }
    } else {
        error!("Unable to connect to database to check table");
    }

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .wrap(middleware::Logger::default())
            .wrap(HttpAuthentication::basic(check_password))
            .configure(app_config)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

fn app_config(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("")
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/submit").route(web::post().to(handle_submit)))
            .service(web::resource("/data").route(web::get().to(get_data_listing)))
            .service(web::resource("/points").route(web::get().to(get_points_chart)))
            .service(web::resource("/accuracy").route(web::get().to(get_accuracy_chart)))
    );
}

// have to do all of this to make a response for if basic authentication fails
#[derive(Clone, Debug)]
struct BasicChallenge();

impl std::fmt::Display for BasicChallenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Basic")
    }
}

impl TryIntoHeaderValue for BasicChallenge {
    type Error = InvalidHeaderValue;
    fn try_into_value(self) -> Result<actix_web::http::header::HeaderValue, Self::Error> {
        HeaderValue::from_bytes(b"Basic")
    }
}

impl Challenge for BasicChallenge {
    fn to_bytes(&self) -> web::Bytes {
        "Basic".try_into_bytes().unwrap()
    }
}

// simple HTTP basic auth password check to protect the website
async fn check_password(req: ServiceRequest, credentials: BasicAuth) -> HttpResult<ServiceRequest> {
    match credentials.password() {
        Some(pass) if pass == "goselkie" => Ok(req),
        _ => Err(AuthenticationError::new(BasicChallenge()).into())
    }
}

// put the form on the main page
async fn index() -> HttpResult<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(include_str!("../static/form.html")))
}

async fn get_data_listing(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
    let data_listing: DataListing = web::block(move || -> Result<DataListing, DatabaseError> {
        use crate::schema::data::dsl::*;
        let conn = pool.get()?;
        let results: Vec<RobotMatchInfo> = data.order_by(team.asc()).load(&conn)?;
        let listing: DataListing = DataListing{ data: results };
        Ok(listing)
    }).await.map_err(ErrorInternalServerError)?.map_err(ErrorInternalServerError)?;

    Ok(
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(data_listing.render().map_err(|e| ErrorInternalServerError(e))?)
    )
}

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

async fn get_accuracy_chart(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
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

async fn get_points_chart(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
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

/// handle POST request to submit data
async fn handle_submit(pool: web::Data<DbPool>, params: web::Form<RobotMatchInfo>) -> HttpResult<HttpResponse> {
    let params = params.into_inner();
    let made = params.teleop_high_made + params.teleop_low_made;
    let missed = params.teleop_high_missed + params.teleop_low_missed;
    let total = made + missed;
    let accuracy = (made as f64) / (total as f64);

    info!("Inserting team {} match {}", params.team, params.match_number);

    use schema::*;

    // run the blocking database tasks (this probably puts it on its own thread)
    web::block(move || -> Result<(), DatabaseError> {
        // just connect to the connection pool and insert the values
        let conn = pool.get()?;
        diesel::insert_into(data::table)
            .values(&params)
            .execute(&conn)?;
        Ok(())
    })
    // just turn everything into internal server errors
    .await.map_err(ErrorInternalServerError)?.map_err(ErrorInternalServerError)?;

    
    Ok(HttpResponse::Found()
        .append_header(("Location", "/")).finish())
}

