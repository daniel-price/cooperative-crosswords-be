extern crate futures;
extern crate serde;

use actix_web::web;
use futures::future;
use itertools::Itertools;
use scraper::Html;
use std::collections::HashMap;
use std::num::ParseIntError;
use uuid::Uuid;

use crate::models::api_models::Cell::{Black, White};
use crate::models::api_models::{Cell, Clue, ClueId, CrosswordDto, Direction};
use crate::models::db_models::Crossword;
use crate::models::errors::AppError;
use crate::models::guardian::{GuardianCrossword, GuardianDirection, GuardianEntry};
use crate::services::crossword_db_actions::{get_crossword_nos_for_series, store_crosswords};
use crate::DbPool;

pub async fn scrape_crossword(series: &str, id: String) -> Result<GuardianCrossword, AppError> {
    let url = format!("https://www.theguardian.com/crosswords/{}/{}", series, id);
    let document = get_document(url).await?;
    let selector = scraper::Selector::parse(".js-crossword")?;
    let element = document
        .select(&selector)
        .last()
        .map_or(Err("No element found".to_string()), Ok)?;
    let json = element
        .value()
        .attr("data-crossword-data")
        .map_or(Err("No attribute found".to_string()), Ok)?;
    let result = serde_json::from_str(&json)?;
    Ok(result)
}

async fn get_document(url: String) -> Result<Html, AppError> {
    let response = reqwest::get(url).await?.text().await?;
    Ok(Html::parse_document(&response))
}

async fn get_recent_crossword_nos(series: &str) -> Result<Vec<i64>, AppError> {
    let url = format!("https://www.theguardian.com/crosswords/series/{}", series);
    let series_url = format!("https://www.theguardian.com/crosswords/{}", series);
    let document = get_document(url).await?;
    let selector = scraper::Selector::parse(".fc-item__container>a")?;
    let crossword_nos: Result<Vec<i64>, ParseIntError> = document
        .select(&selector)
        .map(|s| s.value().attr("href"))
        .flatten()
        .map(|s| s.to_string())
        .filter(|url| url.starts_with(series_url.as_str()))
        .map(|url| {
            url.as_str()
                .replace(series_url.as_str(), "")
                .replace("/", "")
                .parse::<i64>()
        })
        .collect();
    crossword_nos.map_err(|e| AppError::InternalServerError(e.to_string()))
}

pub async fn update_crosswords(pool: web::Data<DbPool>) -> Result<String, AppError> {
    let series = "cryptic";
    let new_crossword_nos: Vec<i64> = get_recent_crossword_nos(series).await?;
    let existing_crosswords_nos: Vec<i64> =
        get_crossword_nos_for_series(pool.clone(), series.to_string()).await?;

    let new_crosswords: Result<Vec<Crossword>, serde_json::Error> = future::try_join_all(
        new_crossword_nos
            .iter()
            .filter(|crossword_id| !existing_crosswords_nos.contains(crossword_id))
            .map(|crossword_id| scrape_crossword(series, crossword_id.to_string())),
    )
    .await?
    .iter()
    .map(|guardian_crossword| {
        serde_json::to_value(guardian_crossword).map(|json_value| Crossword {
            id: Uuid::new_v4().to_string(),
            series: series.to_string(),
            series_no: guardian_crossword.number,
            crossword_json: json_value,
            date: guardian_crossword.date,
        })
    })
    .collect();
    let updated_crosswords = store_crosswords(pool.clone(), new_crosswords?).await?;
    Ok(format!(
        "Successfully scraped {} new crosswords",
        updated_crosswords.to_string()
    ))
}

pub fn guardian_to_crossword_dto(guardian_crossword: GuardianCrossword) -> CrosswordDto {
    let (across, down): (Vec<GuardianEntry>, Vec<GuardianEntry>) = guardian_crossword
        .clone()
        .entries
        .into_iter()
        .partition(|n| n.direction == GuardianDirection::Across);
    fn to_clues(entries: Vec<GuardianEntry>, direction: String) -> Vec<Clue> {
        entries
            .iter()
            .map(|entry| Clue {
                number: entry.number,
                text: entry.clone().clue,
                direction: direction.clone(),
                length: vec![entry.length],
            })
            .collect()
    }
    let index_to_clue_items: HashMap<i64, Vec<(ClueId, Option<i64>)>> = guardian_crossword
        .clone()
        .entries
        .iter()
        .flat_map(|x| to_interim_clue(x.clone(), guardian_crossword.dimensions.cols))
        .into_group_map();
    let grid = (0..(guardian_crossword.dimensions.cols * guardian_crossword.dimensions.rows))
        .map(|x| get_cell(index_to_clue_items.get(&x)))
        .collect();

    let mut clues = to_clues(down, "down".to_string());
    let across_clues = to_clues(across, "across".to_string());
    clues.append(&mut across_clues.clone());
    CrosswordDto {
        number_of_columns: guardian_crossword.dimensions.cols,
        number_of_rows: guardian_crossword.dimensions.rows,
        cells: grid,
        clues,
    }
}

fn to_interim_clue(entry: GuardianEntry, columns: i64) -> Vec<(i64, (ClueId, Option<i64>))> {
    let clue_id = ClueId {
        number: entry.number,
        direction: guardian_to_dto_direction(entry.clone().direction),
    };
    let initial_index = entry.position.x + entry.position.y * columns;
    let increment = match clue_id.direction {
        Direction::Across => 1,
        Direction::Down => columns,
    };
    let first_position = (initial_index, (clue_id.clone(), Some(entry.number)));
    let mut other_positions: Vec<(i64, (ClueId, Option<i64>))> = (1..entry.length)
        .map(|i| (initial_index + i * increment, (clue_id.clone(), None)))
        .collect();
    other_positions.push(first_position);
    other_positions
}
fn guardian_to_dto_direction(direction: GuardianDirection) -> Direction {
    match direction {
        GuardianDirection::Across => Direction::Across,
        GuardianDirection::Down => Direction::Down,
    }
}

fn get_cell(clue_items: Option<&Vec<(ClueId, Option<i64>)>>) -> Cell {
    match clue_items.clone() {
        None => Black,
        Some(clues) => {
            let first_clue = clues.get(0);
            let second_clue = clues.get(1);
            let number = first_clue
                .and_then(|&(_, n)| n)
                .or_else(|| second_clue.and_then(|&(_, n)| n));
            first_clue.map(|_| White { number }).unwrap_or(Black)
        }
    }
}
