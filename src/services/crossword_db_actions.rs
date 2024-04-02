#![allow(unused)]

use actix_web::web;
use diesel::row::NamedRow;
use diesel::{ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl};
use serde_json::Value;

use crate::models::api_models::{Clue, CrosswordDto, CrosswordMetadata};
use crate::models::db_models::Crossword;
use crate::models::errors::AppError;
use crate::models::errors::AppError::InternalServerError;
use crate::models::guardian::{GuardianCrossword, GuardianEntry};
use crate::schema::crossword::dsl::{crossword, crossword_json, date, id, series, series_no};
use crate::services::crossword_service::guardian_to_crossword_dto;
use crate::DbPool;

pub async fn get_crossword_nos_for_series(
    pool: web::Data<DbPool>,
    series_for: String,
) -> actix_web::Result<Vec<i64>, AppError> {
    // use web::block to offload blocking Diesel queries without blocking server thread
    web::block(move || {
        let mut conn = pool.get()?;
        crossword
            .filter(series.eq(series_for))
            .select(series_no)
            .load(&mut conn)
            .map_err(|e| AppError::InternalServerError(e.to_string()))
    })
    .await?
}

pub async fn get_crossword_metadata_for_series(
    pool: web::Data<DbPool>,
    series_for: String,
) -> actix_web::Result<Vec<CrosswordMetadata>, AppError> {
    // use web::block to offload blocking Diesel queries without blocking server thread
    web::block(move || {
        let mut conn = pool.get()?;
        crossword
            .filter(series.eq(series_for))
            .select((id, series, series_no, date))
            .load(&mut conn)
            .map_err(|e| AppError::InternalServerError(e.to_string()))
    })
    .await?
}

pub async fn get_crossword_for_series_and_id(
    pool: web::Data<DbPool>,
    id_for: String,
    series_for: String,
) -> actix_web::Result<CrosswordDto, AppError> {
    // use web::block to offload blocking Diesel queries without blocking server thread
    let result: Value = web::block(move || {
        let mut conn = pool.get()?;
        crossword
            .filter(id.eq(id_for.clone()))
            .filter(series.eq(series_for))
            .select(crossword_json)
            .first(&mut conn)
            .map_err(|_| AppError::CrosswordNotFound(id_for.clone()))
    })
    .await??;
    let guardian_crossword: GuardianCrossword = serde_json::from_value(result)?;
    Ok(guardian_to_crossword_dto(guardian_crossword))
}

pub async fn store_crosswords(
    pool: web::Data<DbPool>,
    crosswords: Vec<Crossword>,
) -> actix_web::Result<usize, AppError> {
    // use web::block to offload blocking Diesel queries without blocking server thread1
    web::block(move || {
        let mut conn = pool.get()?;
        diesel::insert_into(crossword)
            .values(crosswords)
            .execute(&mut conn)
            .map_err(|e| AppError::InternalServerError(e.to_string()))
    })
    .await?
}
