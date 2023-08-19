use std::env;
use std::{path::PathBuf, str::FromStr};
use tracing::info;

mod sql;
use workout_gpx_rs::{load_gpx, Workout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let arg1 = env::args().nth(1);
    let base_path = arg1.expect("usage: workout_gpx_rs PATH");
    let path = PathBuf::from_str(base_path.as_str()).unwrap();

    sql::create_table();

    let workout: Workout = match load_gpx(path).unwrap() {
        Some(w) => w,
        None => panic!("Unable to load workout"),
    };

    info!(
        "Loaded {} records in workout {:?}",
        workout.records.len(),
        workout.activity
    );
    sql::insert_record(&workout).await.unwrap();

    // if path.is_dir() {
    // 	let files = path.read_dir().unwrap();
    // 	for file in files {
    // 		let path = PathBuf::from_str(file.unwrap().path().to_str().unwrap()).unwrap();
    // 		match lib::load_gpx(path).unwrap() {
    // 			Some(w) => {
    // 				println!("{:?}", w.kind);
    // 				sql::insert_record(w).await.unwrap();
    // 			},
    // 			_ => {
    // 				println!("Unable to parse record");
    // 			},
    // 		}
    // 	}
    // }

    Ok(())
}
