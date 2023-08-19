use chrono::NaiveDateTime;
use tokio_stream::{self as stream, StreamExt};

use tracing::info;
use workout_gpx_rs::Workout;

static DB_NAME: &str = "workouts.sqlite";
static TABLE: &str = "workouts";

pub fn create_table() {
    let connection = sqlite::open(String::from(DB_NAME)).unwrap();
    connection
        .execute(
            "
        CREATE TABLE IF NOT EXISTS workouts (
            activity TEXT,
            ds INTEGER,
            ts TEXT,
            lat FLOAT,
            lng FLOAT,
            elevation FLOAT,
            heartrate INTEGER,
            temperature INTEGER,
            UNIQUE(activity, ts, lat, lng)
        );
        ",
        )
        .unwrap();
}

pub async fn insert_record(
    workout: &Workout,
) -> Result<bool, Box<dyn std::error::Error + std::marker::Send + 'static>> {
    let mut count: i32 = 0;
    let connection = sqlite::open(DB_NAME).unwrap();
    let mut stream = stream::iter(&workout.records);

    while let Some(record) = stream.next().await {
        let query = format!(
            "INSERT OR IGNORE INTO {TABLE} (
                    activity,
                    ds,
                    ts,
                    lat,
                    lng,
                    elevation,
                    heartrate,
                    temperature
                 ) VALUES ('{:?}', {:?}, '{:?}', {:?}, {:?}, {:?}, {:?}, {:?})",
            &workout.activity,
			&workout.timestamp,
            &record.timestamp,
            &record.geopoint.lat,
            &record.geopoint.lng,
            &record.elevation,
            &record.heartrate,
            &record.temperature
        );
        connection.execute(&query).expect("Error inserting record");
        count += 1;
    }
    info!(
        "Inserted {} recordings from workout {:?} on {}",
        count, &workout.activity, NaiveDateTime::from_timestamp(workout.timestamp, 0)
    );
    Ok(true)
}
