#![allow(unused)]
#![allow(clippy::all)]

use crate::schema::crossword;
use chrono::NaiveDate;
use diesel::Queryable;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Queryable)]
#[serde(rename_all = "camelCase")]
pub struct CrosswordMetadata {
    pub id: String,
    pub series: String,
    pub series_no: i64,
    pub date: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, Queryable)]
#[serde(rename_all = "camelCase")]
pub struct CrosswordMetadataWithHumanDate {
    pub id: String,
    pub series: String,
    pub series_no: i64,
    pub date: i64,
    pub human_date: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolutionItemDto {
    pub x: i64,
    pub y: i64,
    pub value: String,
    pub modified_by: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable)]
#[serde(rename_all = "camelCase")]
pub struct CrosswordDto {
    pub number_of_columns: i64,
    pub number_of_rows: i64,
    pub cells: Vec<Cell>,
    pub clues: Vec<Clue>,
    pub series: String,
    pub series_no: String,
    pub date: String,
    pub setter: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable)]
#[serde(rename_all = "camelCase")]
pub struct ClueId {
    pub number: i64,
    pub direction: Direction,
    pub solution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable)]
#[serde(rename_all = "camelCase")]
pub struct Clue {
    pub number: i64,
    pub text: String,
    pub direction: String,
    pub length: Vec<i64>,
    pub solution: Option<String>,
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum Direction {
    Across,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Cell {
    Black,
    #[serde(rename_all = "camelCase")]
    White {
        number: Option<i64>,
        letter: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Queryable)]
#[serde(rename_all = "camelCase")]
pub struct CellData {
    pub number: Option<i64>,
    pub clue_id: ClueId,
    pub clue_id_2: Option<ClueId>,
}
