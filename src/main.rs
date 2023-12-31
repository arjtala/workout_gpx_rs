use std::{path::PathBuf, str::FromStr};
use std::num::ParseIntError;

use clap::Parser;
use sha256::digest;
use sqlx::{Pool, Sqlite};
use tracing::info;

mod sql;
use workout_gpx_rs::{load_gpx, Workout};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path of GPX files
    #[arg(short, long)]
    path: Option<String>,

    /// Create table in SQLite
    #[arg(short, long, default_value_t = false)]
    create_table: bool,

    /// Level of concurrency to use for insertions
    #[arg(long, default_value_t = 4)]
    concurrency: usize,

    /// Test hashing
    #[arg(short, long, default_value_t = false)]
    test_hash: bool,
}

fn decode_hex(s: &str, step: usize) -> Result<Vec<u32>, ParseIntError> {
    (0..s.len())
        .step_by(step)
        .map(|i| u32::from_str_radix(&s[i..i + step], 16))
        .collect()
}

fn test_hash() -> Result<(), Box<dyn std::error::Error>> {
    let input = "hello";
    let val = digest(input);
    let rs: Vec<u32> = decode_hex(&val, 8)?;
    let snum: String = rs.iter().map(ToString::to_string).collect();
    // let num: u128 = snum.parse()?;
    info!("Input: {}, digest: {}, large number: {}", input, val, snum);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    if args.test_hash {
        return test_hash();
    }

    let sqlite_pool: Pool<Sqlite> = sql::get_sqlite_pool(args.concurrency).await?;
    if args.create_table {
        info!("Creating new table in SQLite database");
        sql::create_table(&sqlite_pool).await?;
    }

    let mut workouts: Vec<Workout> = Vec::new();

    if let Some(p) = args.path {
        let path = PathBuf::from_str(p.as_str())?;
        info!(
            "Path [{:?}] is directory: {} or file: {}",
            path,
            path.is_dir(),
            path.is_file()
        );

        if path.is_file() {
            let workout: Workout = match load_gpx(path).unwrap() {
                Some(w) => w,
                None => panic!("Unable to load workout"),
            };

            info!(
                "Loaded {} records in workout {:?}",
                workout.records.len(),
                workout.activity
            );
            workouts.push(workout);
        } else if path.is_dir() {
            let files = path.read_dir().unwrap();
            for file in files {
                let path = PathBuf::from_str(file.unwrap().path().to_str().unwrap()).unwrap();
                match load_gpx(path).unwrap() {
                    Some(w) => {
                        workouts.push(w);
                    }
                    _ => {
                        println!("Unable to parse record");
                    }
                }
            }
        }
    }
    info!("Loaded {} workouts to insert", workouts.len());
    sql::insert_records(&sqlite_pool, workouts, args.concurrency).await?;

    Ok(())
}
