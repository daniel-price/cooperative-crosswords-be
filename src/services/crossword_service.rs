extern crate futures;
extern crate serde;

use actix_web::web;
use futures::future;
use itertools::Itertools;
use reqwest::header::{HeaderMap, HeaderValue};
use scraper::Html;
use std::collections::HashMap;
use std::error::Error;
use std::num::ParseIntError;
use uuid::Uuid;

use crate::models::api_models::Cell::{Black, White};
use crate::models::api_models::{Cell, Clue, ClueId, CrosswordDto, Direction};
use crate::models::db_models::Crossword;
use crate::models::errors::AppError;
use crate::models::guardian::{
    GuardianCrossword, GuardianCrosswordData, GuardianDirection, GuardianEntry,
};
use crate::services::crossword_db_actions::{get_crossword_nos_for_series, store_crosswords};
use crate::services::util::to_human_readable_date;
use crate::DbPool;

type InterimClue = (ClueId, Option<i64>, String);

pub async fn scrape_crossword(series: &str, id: String) -> Result<GuardianCrossword, AppError> {
    println!("Scraping {series} crossword: {id}",);
    let url = format!("https://www.theguardian.com/crosswords/{}/{}", series, id);
    let document = get_document(url).await?;
    let selector = scraper::Selector::parse("[name=CrosswordComponent]")?;
    let element = document.select(&selector).last();
    match element {
        Some(e) => {
            let json = e
                .value()
                .attr("props")
                .map_or(Err("No attribute found".to_string()), Ok)?;
            let result: GuardianCrosswordData = serde_json::from_str(json)?;
            Ok(result.data)
        }
        None => {
            println!("Failed to scrape {series} crossword: {id}",);

            Err(AppError::InternalServerError(
                "No crossword found".to_string(),
            ))
        }
    }
}

fn construct_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
    headers.insert(
        "accept-language",
        HeaderValue::from_static("en-GB,en-US;q=0.9,en;q=0.8"),
    );
    headers.insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Linux; Android 6.0; Nexus 5 Build/MRA58N) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/132.0.0.0 Mobile Safari/537.36"),
    );
    headers
}

async fn get_document(url: String) -> Result<Html, AppError> {
    let client = reqwest::Client::new();

    let response1 = client.get(url).headers(construct_headers()).send().await;
    match response1 {
        Ok(r) => {
            let response = r.text().await?;
            Ok(Html::parse_document(&response))
        }
        Err(e) => {
            println!("Error: {} - {:#?}", e, e.source());
            Err(AppError::InternalServerError(e.to_string()))
        }
    }
}

async fn get_recent_crossword_nos(series: &str, page: &i32) -> Result<Vec<i64>, AppError> {
    let relative_url = format!("/crosswords/{}", series);
    let url = format!("https://www.theguardian.com/crosswords/series/{series}?page={page}",);
    let selector = scraper::Selector::parse("a")?;
    let document = get_document(url).await;
    match document {
        Ok(doc) => {
            let a = doc.select(&selector);

            let crossword_nos: Result<Vec<i64>, ParseIntError> = a
                .map(|s| s.value().attr("href"))
                .flatten()
                .map(|s| s.to_string())
                .filter(|url| url.starts_with(relative_url.as_str()) && !url.ends_with("#comments"))
                .map(|url| {
                    url.as_str()
                        .replace(relative_url.as_str(), "")
                        .replace("/", "")
                        .parse::<i64>()
                })
                .collect();

            crossword_nos.map_err(|e| AppError::InternalServerError(e.to_string()))
        }
        Err(e) => {
            println!("Error: {}", e);
            Err(e)
        }
    }
}

pub async fn bulk_update_series(
    pool: web::Data<DbPool>,
    series: &str,
    from_id: &i64,
    to_id: &i64,
) -> Result<String, AppError> {
    let existing_crosswords_nos: Vec<i64> =
        get_crossword_nos_for_series(pool.clone(), series.to_string()).await?;

    for id in *from_id..*to_id {
        if existing_crosswords_nos.contains(&id) {
            println!("Crossword {} already exists", id);
            continue;
        }
        let result = scrape_crossword(series, id.to_string()).await;
        match result {
            Ok(guardian_crossword) => {
                let crossword =
                    serde_json::to_value(guardian_crossword.clone()).map(|json_value| Crossword {
                        id: Uuid::new_v4().to_string(),
                        series: series.to_string(),
                        series_no: guardian_crossword.number,
                        crossword_json: json_value,
                        date: guardian_crossword.date,
                    });

                match crossword {
                    Ok(crossword) => {
                        store_crosswords(pool.clone(), vec![crossword]).await?;
                        println!("Successfully stored {series} crossword {id}",);
                    }
                    Err(e) => {
                        println!("Error storing {series} crossword {id}: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Error scraping {series} crossword {id}: {}", e);
            }
        }
    }
    Ok("Successfully scraped new crosswords".to_string())
}

pub async fn update_crosswords(
    pool: web::Data<DbPool>,
    series: &str,
    page: &i32,
) -> Result<String, AppError> {
    let new_crossword_nos: Vec<i64> = get_recent_crossword_nos(series, page).await?;
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
                solution: entry.solution.clone(),
            })
            .collect()
    }
    let index_to_clue_items_and_letter: HashMap<i64, Vec<InterimClue>> = guardian_crossword
        .clone()
        .entries
        .iter()
        .flat_map(|x| to_interim_clue(x.clone(), guardian_crossword.dimensions.cols))
        .into_group_map();

    let grid = (0..(guardian_crossword.dimensions.cols * guardian_crossword.dimensions.rows))
        .map(|x| get_cell(index_to_clue_items_and_letter.get(&x)))
        .collect();

    let mut clues = to_clues(down, "down".to_string());
    let across_clues = to_clues(across, "across".to_string());
    clues.append(&mut across_clues.clone());
    CrosswordDto {
        number_of_columns: guardian_crossword.dimensions.cols,
        number_of_rows: guardian_crossword.dimensions.rows,
        cells: grid,
        clues,
        series: guardian_crossword.crossword_type,
        series_no: guardian_crossword.number.to_string(),
        date: to_human_readable_date(guardian_crossword.date),
        setter: guardian_crossword
            .creator
            .map_or("".to_string(), |c| c.name),
    }
}

fn to_interim_clue(entry: GuardianEntry, columns: i64) -> Vec<(i64, InterimClue)> {
    print!("Entry: {:#?}", entry);
    let solution = entry.solution.clone();
    let clue_id = ClueId {
        number: entry.number,
        direction: guardian_to_dto_direction(entry.clone().direction),
        solution: entry.solution,
    };
    let initial_index = entry.position.x + entry.position.y * columns;
    let increment = match clue_id.direction {
        Direction::Across => 1,
        Direction::Down => columns,
    };

    let interim_clue = (
        clue_id.clone(),
        Some(entry.number),
        match solution.clone() {
            Some(s) => s.chars().next().unwrap().to_string(),
            None => "".to_string(),
        },
    );
    let first_position = (initial_index, interim_clue);
    let mut other_positions: Vec<(i64, InterimClue)> = (1..entry.length)
        .map(|i| {
            let other_index = initial_index + i * increment;
            let interim_clue = (
                clue_id.clone(),
                None,
                match solution.clone() {
                    Some(s) => s.chars().nth(i as usize).unwrap().to_string(),
                    None => "".to_string(),
                },
            );
            (other_index, interim_clue)
        })
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

fn get_cell(clue_items: Option<&Vec<InterimClue>>) -> Cell {
    match clue_items.clone() {
        None => Black,
        Some(clues) => {
            let first_clue = clues.get(0);
            let second_clue = clues.get(1);
            let number = first_clue
                .and_then(|(_, n, _s)| n.clone())
                .or_else(|| second_clue.and_then(|(_, n, _s)| n.clone()));
            first_clue
                .map(|(_, _, s)| White {
                    number,
                    letter: s.to_string(),
                })
                .unwrap_or(Black)
        }
    }
}
