use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::NaiveDateTime;
use lazy_static::lazy_static;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use strum::{EnumString, EnumVariantNames, VariantNames};
use tracing::info;
use xml::reader::{EventReader, XmlEvent};

#[derive(EnumString, EnumVariantNames, Debug, Serialize, Deserialize)]
pub enum Activity {
    Running,
    Cycling,
    Unknown,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Record {
    #[serde(flatten)]
    pub geopoint: GeoPoint,
    pub elevation: f32,
    pub timestamp: String,
    pub heartrate: i32,
    pub temperature: i32,
    pub speed: f32,
    pub course: f32,
    pub hAcc: f32,
    pub vAcc: f32,
}

impl Record {
    fn load_data(&mut self, data: &HashMap<String, String>) {
        if let Some(v) = data.get("ele") {
            self.elevation = v.parse::<f32>().unwrap();
        }
        if let Some(v) = data.get("time") {
            self.timestamp = v.clone();
        }
        if let Some(v) = data.get("hr") {
            self.heartrate = v.parse::<i32>().unwrap();
        }
        if let Some(v) = data.get("atemp") {
            self.temperature = v.parse::<i32>().unwrap();
        }
        if let Some(v) = data.get("speed") {
            self.speed = v.parse::<f32>().unwrap();
        }
        if let Some(v) = data.get("course") {
            self.course = v.parse::<f32>().unwrap();
        }
        if let Some(v) = data.get("hAcc") {
            self.hAcc = v.parse::<f32>().unwrap();
        }
        if let Some(v) = data.get("vAcc") {
            self.vAcc = v.parse::<f32>().unwrap();
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Workout {
    #[serde(flatten)]
    pub activity: Activity,
    pub timestamp: i64,
    pub records: Vec<Record>,
}

lazy_static! {

    // A Regular Expression used to find variant names in target strings.
    //
    static ref ACTIVITY_EXPR: Regex = {

        // Piece together the expression from Thing's variant names.
        let expr_str = Activity::VARIANTS.join("|");

        Regex::new(&expr_str).unwrap()
    };
}

pub fn get_activity(path: &str) -> anyhow::Result<Activity, Box<dyn std::error::Error>> {
    if let Some(captures) = ACTIVITY_EXPR.captures(path) {
        let name = &captures[0];
        Ok(Activity::from_str(name).unwrap())
    } else {
        Ok(Activity::Unknown)
    }
}

pub fn get_timestamp(path: &str) -> anyhow::Result<NaiveDateTime, Box<dyn std::error::Error>> {
    let re: Regex = Regex::new("[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}").unwrap();
    let d = re.find(path).unwrap().as_str();
    let timestamp = NaiveDateTime::parse_from_str(d, "%Y-%m-%d-%H%M%S")?;
    Ok(timestamp)
}

// #[tracing::instrument]
pub fn load_gpx(path: PathBuf) -> anyhow::Result<Option<Workout>, Box<dyn std::error::Error>> {
    let path_str = path.to_str().ok_or("")?;
    let activity = get_activity(path_str)?;
    let timestamp = get_timestamp(path_str)?;
    info!("Loading activity {:?} from {}", activity, timestamp);
    match activity {
        Activity::Unknown => Ok(None),
        _ => {
            let file = File::open(path).unwrap();
            let file = BufReader::new(file);

            let mut records: Vec<Record> = Vec::new();
            let mut current_element = String::new();
            let mut geopoint = GeoPoint {
                ..Default::default()
            };
            let mut record = Record {
                ..Default::default()
            };

            let parser = EventReader::new(file);
            for event in parser {
                match event? {
                    XmlEvent::StartElement {
                        name, attributes, ..
                    } => {
                        current_element = name.local_name;
                        if current_element.as_str() == "trkpt" {
                            for attr in attributes {
                                match attr.name.local_name.as_str() {
                                    "lat" => geopoint.lat = attr.value.parse::<f64>()?,
                                    "lon" => geopoint.lng = attr.value.parse::<f64>()?,
                                    _ => (),
                                }
                            }
                        }
                    }
                    XmlEvent::Characters(text) => {
                        let map = HashMap::from([(current_element.clone(), text.clone())]);
                        record.load_data(&map);
                    }
                    _ => (),
                }
                record.geopoint = geopoint.clone();
                records.push(record.clone());
            }
            Ok(Some(Workout {
                records,
                activity,
                timestamp: timestamp.timestamp(),
            }))
        }
    }
}
