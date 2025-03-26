use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianCrosswordData {
    pub data: GuardianCrossword,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianCrossword {
    pub id: String,
    pub number: i64,
    pub name: String,
    pub creator: Option<GuardianCreator>,
    pub date: i64,
    pub web_publication_date: i64,
    pub entries: Vec<GuardianEntry>,
    pub solution_available: bool,
    pub date_solution_available: i64,
    pub dimensions: Dimensions,
    pub crossword_type: String,
    pub pdf: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianCreator {
    pub name: String,
    pub web_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianEntry {
    pub id: String,
    pub number: i64,
    pub human_number: String,
    pub clue: String,
    pub direction: GuardianDirection,
    pub length: i64,
    pub group: Vec<String>,
    pub position: Position,
    pub separator_locations: SeparatorLocations,
    pub solution: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GuardianDirection {
    #[serde(rename = "across")]
    Across,
    #[serde(rename = "down")]
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeparatorLocations {
    #[serde(rename = ",")]
    #[serde(default)]
    pub field: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dimensions {
    pub cols: i64,
    pub rows: i64,
}
