use std::fs;
use std::str::FromStr;
use std::time::Duration;

use chrono::NaiveDateTime;
use const_format::formatcp;
use futures::future::join_all;
use futures::stream::{self, StreamExt, TryStreamExt};
use rusqlite::{params, Connection};
use sqlx::{
    migrate::MigrateDatabase,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    Pool, Sqlite,
};
use tracing::info;
use workout_gpx_rs::Workout;

const PRAGMAS: [&str; 5] = [
    "PRAGMA journal_mode = OFF",
    "PRAGMA synchronous = 0",
    "PRAGMA cache_size = 1000000",
    "PRAGMA locking_mode = EXCLUSIVE",
    "PRAGMA temp_store = MEMORY",
    // "PRAGMA foreign_keys;",
    // "PRAGMA TEMP_STORE = MEMORY;",
    // "PRAGMA MMAP_SIZE = 30000000000;",
    // "PRAGMA PAGE_SIZE = 4096;",
    // "PRAGMA foreign_keys;",
];

const DB_NAME: &'static str = "workouts.sqlite";
const TABLE: &'static str = "workouts";
const INSERT_SQL: &'static str = formatcp!(
    "INSERT OR IGNORE INTO {} (
    activity,
    ds,
    ts,
    lat,
    lng,
    elevation,
    heartrate,
    temperature
    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
    TABLE
);
const GEO_SQL: &'static str = formatcp!(
    "INSERT INTO {}_geo (
    activity,
    ds,
    _shape
    )  VALUES(?, ?, ?);",
    TABLE
);

pub async fn get_sqlite_pool(
    concurrency: usize,
) -> Result<Pool<Sqlite>, Box<dyn std::error::Error>> {
    let database_url: String = format!("sqlite://{}", DB_NAME);
    let pool_timeout: Duration = Duration::from_secs(30);
    let pool_max_connections: u32 = if concurrency == 1 {
        2
    } else {
        concurrency as u32
    };
    info!(
        "Setting up configuration with {} concurrent connections",
        concurrency
    );
    let connection_options = SqliteConnectOptions::from_str(&database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Off)
        .synchronous(SqliteSynchronous::Off)
        .busy_timeout(pool_timeout);

    info!("Creating connection pool");
    let sqlite_pool: Pool<Sqlite> = SqlitePoolOptions::new()
        .max_connections(pool_max_connections)
        .idle_timeout(pool_timeout)
        .connect_with(connection_options)
        .await?;

    info!("Executing {} PRAGMA statements", PRAGMAS.len());
    join_all(
        PRAGMAS
            .into_iter()
            .map(|p| sqlx::query(p).execute(&sqlite_pool)),
    )
    .await;
    info!("Connection created");
    Ok(sqlite_pool)
}

pub async fn create_table(sqlite_pool: &Pool<Sqlite>) -> Result<(), Box<dyn std::error::Error>> {
    info!("Removing existing db file: {}", DB_NAME);
    let _ = fs::remove_file(DB_NAME);
    info!("Executing table creation");
    sqlx::migrate!("./db").run(sqlite_pool).await?;
    Ok(())
}

pub async fn insert_records(
    sqlite_pool: &Pool<Sqlite>,
    workouts: Vec<Workout>,
    concurrency: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = stream::iter(workouts);
    stream
        .map(Ok::<workout_gpx_rs::Workout, Box<dyn std::error::Error>>) // Optional, but can help with type inference
        .try_for_each_concurrent(concurrency, |w| async move {
            let activity = w.activity.to_string();
            let geopoly = w.geopoly();
            for record in w.records {
                if record.validate()? {
                    let (lat, lng) = record
                        .geopoint
                        .as_ref()
                        .map_or((0.0, 0.0), |g| (g.lat, g.lng));
                    let _ = sqlx::query(&INSERT_SQL)
                        .bind(record.ds)
                        .bind(record.timestamp)
                        .bind(lat)
                        .bind(lng)
                        .bind(record.elevation)
                        .bind(record.heartrate)
                        .bind(record.temperature)
                        .execute(sqlite_pool)
                        .await?;
                }
            }
            match geopoly {
                Ok(coords) => {
                    sqlx::query(&GEO_SQL)
                        .bind(activity)
                        .bind(w.timestamp)
                        .bind(coords)
                        .execute(sqlite_pool)
                        .await?;
                }
                Err(_) => {
                    return Err("Unable to insert geospatial coordinates".into());
                }
            }
            Ok(())
        })
        .await?;
    Ok(())
}
