// using macro_use syntax because a normal use didn't seem to be doing it
// should probably figure out why and `use` the correct macros
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate migrations_macros;
use actix_web::error::ErrorInternalServerError;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result as HttpResult, dev::ServiceRequest, error::ErrorForbidden};
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

// simple HTTP basic auth password check to protect the website
async fn check_password(req: ServiceRequest, credentials: BasicAuth) -> HttpResult<ServiceRequest> {
    match credentials.password() {
        Some(pass) if pass == "abc" => Ok(req),
        _ => Err(ErrorForbidden("Wrong password!"))
    }
}

fn app_config(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("")
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/submit").route(web::post().to(handle_submit)))
            .service(web::resource("/data").route(web::get().to(get_data_listing)))
    );
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
        let results: Vec<RobotMatchInfo> = data.load(&conn)?;
        let listing: DataListing = DataListing{ data: results };
        Ok(listing)
    }).await.map_err(ErrorInternalServerError)?.map_err(ErrorInternalServerError)?;

    Ok(
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(data_listing.render().map_err(|e| ErrorInternalServerError(e))?)
    )
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
