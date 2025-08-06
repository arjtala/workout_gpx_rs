use const_format::formatcp;
use rusqlite::{Connection, ToSql};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Receiver;
use tracing::info;
use workout_gpx_rs::Workout;

const PRAGMAS: [&str; 5] = [
    "PRAGMA journal_mode = OFF",
    "PRAGMA synchronous = 0",
    "PRAGMA cache_size = 1000000",
    "PRAGMA locking_mode = EXCLUSIVE",
    "PRAGMA temp_store = MEMORY",
];

const DB_NAME: &str = "workouts.sqlite";
const TABLE: &str = "workouts";
const CREATE_TABLES: &str = formatcp!(
    "DROP TABLE IF EXISTS {};
DROP TABLE IF EXISTS {}_records;
DROP TABLE IF EXISTS {}_geo;
CREATE TABLE IF NOT EXISTS {}_records (
  wid integer,
  ds integer,
  ts text,
  lat float,
  lng float,
  elevation float,
  heartrate integer,
  temperature integer,
  UNIQUE (wid, ds, ts)
);
CREATE TABLE IF NOT EXISTS {} (
  wid integer PRIMARY KEY UNIQUE NOT NULL,
  activity text,
  ds integer,
  record_locations text
);
CREATE VIRTUAL TABLE {}_geo
USING geopoly ();",
    TABLE,
    TABLE,
    TABLE,
    TABLE,
    TABLE,
    TABLE
);

const WORKOUT_SQL: & str = formatcp!(
	"INSERT OR IGNORE INTO {} (
    wid,
    activity,
    ds,
    record_locations
    ) VALUES (?, ?, ?, ?);",
	TABLE
);
const RECORD_SQL: &str = formatcp!(
    "INSERT OR IGNORE INTO {}_records (
    wid,
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
const GEO_SQL: &str = formatcp!(
    "INSERT INTO {}_geo (
    activity,
    ds,
    _shape
    )  VALUES(?, ?, ?);",
    TABLE
);

pub fn get_connection() -> Result<Connection, Box<dyn std::error::Error>> {
    let conn: Connection = Connection::open(DB_NAME)?;
    info!("Executing {} PRAGMA statements", PRAGMAS.len());
    conn.execute_batch(&PRAGMAS.join("; ")).expect("PRAGMAS");
    info!("Connection created");
    Ok(conn)
}

pub async fn create_table(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    // info!("Removing existing db file: {}", DB_NAME);
    // let _ = fs::remove_file(DB_NAME);
    info!("Executing table creation");
    conn.execute_batch(CREATE_TABLES)?;
    Ok(())
}

pub async fn insert_records(
    conn: &mut Connection,
    mut workouts: Receiver<Workout>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let tx = conn.transaction().unwrap();
        {
            let mut stmt_record = tx.prepare_cached(RECORD_SQL)?;
            let mut stmt_geo = tx.prepare_cached(GEO_SQL)?;
            let mut stmt_workout = tx.prepare_cached(WORKOUT_SQL)?;

            match workouts.try_recv() {
                Ok(w) => {
                    let activity = w.activity.to_string();
                    let geopoly = w.geopoly();
                    let row_values: Vec<&dyn ToSql> = vec![];
                    for record in w.records {
                        if record.validate()? {
                            let mut row_values: Vec<&dyn ToSql> = Vec::new();
                            let (lat, lng) = record
                                .geopoint
                                .as_ref()
                                .map_or((0.0, 0.0), |g| (g.lat, g.lng));
                            row_values.push(&record.ds as &dyn ToSql);
                            row_values.push(&record.timestamp as &dyn ToSql);
                            row_values.push(&lat as &dyn ToSql);
                            row_values.push(&lng as &dyn ToSql);
                            row_values.push(&record.elevation as &dyn ToSql);
                            row_values.push(&record.heartrate as &dyn ToSql);
                            row_values.push(&record.temperature as &dyn ToSql);
                        }
                    }
                    stmt_record.execute(&*row_values)?;

                    match geopoly {
                        Ok(coords) => {
                            let row_values: Vec<&dyn ToSql> = vec![
                                &activity as &dyn ToSql,
                                &w.timestamp as &dyn ToSql,
                                &coords as &dyn ToSql,
                            ];
                            stmt_geo.execute(&*row_values)?;
                        }
                        Err(_) => {
                            return Err("Unable to insert geospatial coordinates".into());
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    info!("No more records to process!");
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    info!("The receiver channel has been closed");
                    break;
                }
            }
        }
        tx.commit().unwrap();
    }
    Ok(())
}
