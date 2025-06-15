use crate::models::errors::{to_status_code, AppError};
use actix::{Actor, Addr};
use actix_cors::Cors;
use actix_web::web::{self, Data, Path, Payload};
use actix_web::{
    get, middleware, post, App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_web_actors::ws::start;
use diesel::r2d2;
use diesel::PgConnection;
use models::api_models::{CrosswordMetadata, CrosswordMetadataWithHumanDate};
use serde::{Deserialize, Serialize};
use services::util::to_human_readable_date;

use crate::services::crossword_db_actions::{
    get_crossword_for_series_and_number, get_crossword_metadata_for_series,
};
use crate::services::ws_server::MoveServer;
use crate::services::ws_session::WsSession;

mod models;
mod schema;
mod services;

type DbPool = r2d2::Pool<r2d2::ConnectionManager<PgConnection>>;

static ALL_SERIES: [&str; 6] = ["quiptic", "quick", "cryptic", "speedy", "prize", "everyman"];

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=trace");
    env_logger::init();
    dotenv::dotenv().ok();
    let pool = initialize_db_pool();
    let server = MoveServer::new(pool.clone()).start();
    HttpServer::new(move || {
        App::new()
            .wrap(Cors::default().allow_any_method().allow_any_origin())
            .wrap(middleware::Logger::default())
            .app_data(Data::new(pool.clone()))
            .app_data(Data::new(server.clone()))
            .service(get_crossword_data)
            .service(get_all_crossword_data)
            .service(update_crosswords)
            .service(bulk_update_crosswords)
            .service(update_all_crosswords)
            .service(start_connection)
    })
    .bind(std::env::var("HOST_PORT").unwrap_or("127.0.0.1:8080".to_string()))?
    .run()
    .await
}

#[derive(Serialize, Deserialize, Debug)]
struct PostData {
    series: String,
    start_id: i64,
    end_id: i64,
}

#[post("/bulk-update-crosswords")]
async fn bulk_update_crosswords(pool: Data<DbPool>, data: web::Json<PostData>) -> impl Responder {
    println!("Bulk update request: {:#?}", data);

    let result = services::crossword_service::bulk_update_series(
        pool,
        data.series.as_str(),
        &data.start_id,
        &data.end_id,
    )
    .await;

    match result {
        Ok(message) => HttpResponse::Ok().body(message),
        Err(error) => build_error_response(error),
    }
}

#[post("/update-all-crosswords")]
async fn update_all_crosswords(pool: Data<DbPool>) -> impl Responder {
    let mut success = true;
    for series in ALL_SERIES.iter() {
        let page = 1;
        let result =
            services::crossword_service::update_crosswords(pool.clone(), series, &page).await;
        match result {
            Ok(_) => println!("Updated crosswords for series: {:#?}", series),
            Err(error) => {
                success = false;
                println!("Error updating crosswords for series: {:#?}", error)
            }
        }
    }
    let message = if success {
        "Successfully updated all crosswords"
    } else {
        "Error updating all crosswords"
    };
    HttpResponse::Ok().body(message)
}

#[post("/update-crosswords/{series}")]
async fn update_crosswords(pool: Data<DbPool>, path: Path<(String,)>) -> impl Responder {
    let series = path.into_inner().0;
    let page = 1;
    let result = services::crossword_service::update_crosswords(pool, &series, &page).await;
    match result {
        Ok(message) => HttpResponse::Ok().body(message),
        Err(error) => build_error_response(error),
    }
}

#[get("/crossword/{series}/{seriesNo}")]
async fn get_crossword_data(pool: Data<DbPool>, path: Path<(String, String)>) -> impl Responder {
    let params = path.into_inner();
    let series = params.0;
    let series_no = params.1;
    let crossword_data = get_crossword_for_series_and_number(pool, series_no, series).await;
    match crossword_data {
        Ok(message) => serde_json::to_string(&message).map_or(
            HttpResponse::BadRequest().body("Couldn't parse crossword to a string"),
            |x| HttpResponse::Ok().body(x),
        ),
        Err(error) => build_error_response(error),
    }
}

#[get("/crosswords")]
async fn get_all_crossword_data(pool: Data<DbPool>) -> impl Responder {
    let mut all_crosswords: Vec<CrosswordMetadata> = Vec::new();

    for series in ALL_SERIES.iter() {
        let crossword_result =
            get_crossword_metadata_for_series(pool.clone(), series.to_string()).await;
        match crossword_result {
            Ok(crosswords) => {
                all_crosswords.extend(crosswords);
            }
            Err(error) => {
                println!("Error: {:#?}", error);
            }
        }
    }
    serde_json::to_string(
        &all_crosswords
            .iter()
            .map(|c| CrosswordMetadataWithHumanDate {
                id: c.id.clone(),
                series: c.series.clone(),
                series_no: c.series_no,
                date: c.date,
                human_date: to_human_readable_date(c.date),
            })
            .collect::<Vec<CrosswordMetadataWithHumanDate>>(),
    )
    .map_or(
        HttpResponse::BadRequest().body("Couldn't parse crossword to a string"),
        |x| HttpResponse::Ok().body(x),
    )
}

#[get("/move/{team_id}/{crossword_id}/{user_id}")]
pub async fn start_connection(
    req: HttpRequest,
    stream: Payload,
    path: Path<(String, String, String)>,
    srv: Data<Addr<MoveServer>>,
) -> Result<HttpResponse, Error> {
    let ws = WsSession::new(
        srv.get_ref().clone(),
        path.2.clone(),
        path.clone().0,
        path.1.clone(),
    );
    start(ws, &req, stream)
}

fn build_error_response(error: AppError) -> HttpResponse {
    HttpResponse::build(to_status_code(error.clone())).body(error.clone().to_string())
}

fn initialize_db_pool() -> DbPool {
    let conn_spec = std::env::var("DATABASE_URL").expect("DATABASE_URL should be set");
    println!("Connecting to database at: {}", conn_spec);
    let manager = r2d2::ConnectionManager::<PgConnection>::new(conn_spec);
    r2d2::Pool::builder()
        .build(manager)
        .expect("database URL should be valid path to PostgreSQL database")
}
