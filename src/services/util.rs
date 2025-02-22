use chrono::{DateTime, Datelike, Month};
pub fn to_human_readable_date(date: i64) -> String {
    let datetime = DateTime::from_timestamp(date / 1000, 0).unwrap();

    let date = datetime.date_naive().day();
    let month = Month::try_from(u8::try_from(datetime.month()).unwrap()).unwrap();
    let year = datetime.year();
    format!("{:?} {:?} {:?}", date, month, year)
}
