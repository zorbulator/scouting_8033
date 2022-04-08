use diesel::prelude::*;
use actix_web::{HttpResponse, web, error::ErrorInternalServerError};
use askama::Template;

use crate::{DbPool, HttpResult, DataListing, DatabaseError, RobotMatchInfo};

pub async fn get_data_listing(pool: web::Data<DbPool>) -> HttpResult<HttpResponse> {
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

