use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::NaiveDateTime;
use lazy_static::lazy_static;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use strum::{EnumString, EnumVariantNames, VariantNames};
use xml::reader::{EventReader, XmlEvent};

static EPSILON: f64 = 0.0000001;
const WORKOUT_DATETIME_FMT: &str = "%Y-%m-%d-%H%M%S";
const RECORD_DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S";
const RECORD_TIMESTAMP: &str =
    r"(?<y>\d{4})-(?<m>\d{2})-(?<d>\d{2})T(?<H>\d{2}):(?<M>\d{2}):(?<S>\d{2}).(?<Z>\d{3})Z";
const REGEX_CHARS: &str = "[0-9]{4}-[0-9]{2}-[0-9]{2}-[0-9]{6}";

#[derive(Clone, EnumString, EnumVariantNames, Debug, Serialize, Deserialize)]
pub enum Activity {
    Running,
    Cycling,
    Unknown,
}

impl fmt::Display for Activity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Copy, Clone, Default, Debug, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Record {
    #[serde(flatten)]
    pub activity: Option<Activity>,
    pub ds: Option<i64>,
    pub timestamp: Option<i64>,
    pub geopoint: Option<GeoPoint>,
    pub elevation: Option<f32>,
    pub heartrate: Option<i32>,
    pub temperature: Option<i32>,
    pub speed: Option<f32>,
    pub course: Option<f32>,
    pub hAcc: Option<f32>,
    pub vAcc: Option<f32>,
}

impl Record {
    fn load_data(
        &mut self,
        data: &HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(v) = data.get("ele") {
            self.elevation = Some(v.parse::<f32>()?);
        }
        if let Some(v) = data.get("time") {
            self.timestamp = Some(get_record_timestamp(&v.clone(), RECORD_TIMESTAMP)?);
        }
        if let Some(v) = data.get("hr") {
            self.heartrate = Some(v.parse::<i32>()?);
        }
        if let Some(v) = data.get("atemp") {
            self.temperature = Some(v.parse::<i32>()?);
        }
        if let Some(v) = data.get("speed") {
            self.speed = Some(v.parse::<f32>()?);
        }
        if let Some(v) = data.get("course") {
            self.course = Some(v.parse::<f32>()?);
        }
        if let Some(v) = data.get("hAcc") {
            self.hAcc = Some(v.parse::<f32>()?);
        }
        if let Some(v) = data.get("vAcc") {
            self.vAcc = Some(v.parse::<f32>()?);
        }
        Ok(())
    }

    fn _null_island(&self) -> Result<bool, Box<dyn std::error::Error>> {
        match &self.geopoint {
            Some(g) => Ok((g.lat * g.lat + g.lng * g.lng).sqrt() <= EPSILON),
            None => Ok(false),
        }
    }

    pub fn validate(&self) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(!(self._null_island()?)
            && !(self.elevation.is_none()
                && self.timestamp.is_none()
                && self.heartrate.is_none()
                && self.temperature.is_none()
                && self.speed.is_none()
                && self.course.is_none()
                && self.hAcc.is_none()
                && self.vAcc.is_none()))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workout {
    #[serde(flatten)]
    pub activity: Activity,
    pub timestamp: i64,
    pub records: Vec<Record>,
}

impl Workout {
    pub fn geopoly(&self) -> Result<String, Box<dyn std::error::Error>> {
        let coords: Vec<String> = self
            .records
            .iter()
            .filter_map(|record| {
                record.validate().ok().map(|_| match &record.geopoint {
                    Some(g) => format!("[{},{}]", g.lat, g.lng),
                    None => String::from("[0.0,0.0]"),
                })
            })
            .collect();
        let mut result: String = "'[ ".to_owned();
        let suffix: &str = "]'";
        result.push_str(&coords.join(","));
        result.push_str(suffix);
        Ok(result)
    }
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

pub fn get_activity(path: &str) -> Result<Activity, Box<dyn std::error::Error>> {
    if let Some(captures) = ACTIVITY_EXPR.captures(path) {
        let name = &captures[0];
        Ok(Activity::from_str(name)?)
    } else {
        Ok(Activity::Unknown)
    }
}

pub fn get_workout_timestamp(path: &str, regex: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let re: Regex = Regex::new(regex)?;
    let d = re.find(path).ok_or("No match found")?.as_str();
    let timestamp = NaiveDateTime::parse_from_str(d, WORKOUT_DATETIME_FMT)?;
	Ok(timestamp.timestamp())
}


pub fn get_record_timestamp(time: &str, regex: &str) -> Result<i64, Box<dyn std::error::Error>> {
    let re: Regex = Regex::new(regex)?;
    let d = re.replace_all(time, "$y-$m-$d $H:$M:$S");
    let timestamp = NaiveDateTime::parse_from_str(&d, RECORD_DATETIME_FMT)?;
    Ok(timestamp.timestamp())
}

#[tracing::instrument]
pub fn load_gpx(path: PathBuf) -> Result<Option<Workout>, Box<dyn std::error::Error>> {
    let path_str = path.to_str().ok_or("")?;
    let activity = get_activity(path_str)?;
    let timestamp = get_workout_timestamp(path_str, REGEX_CHARS)?;
    match activity {
        Activity::Unknown => Ok(None),
        _ => {
            let file = File::open(path)?;
            let file = BufReader::new(file);

            let mut records: Vec<Record> = Vec::new();
            let mut current_element = String::new();
            let mut record = Record {
                ..Default::default()
            };
            let parser = EventReader::new(file);
            for event in parser {
                let mut geopoint = GeoPoint {
                    ..Default::default()
                };
                match event {
                    Ok(XmlEvent::StartElement {
                        name, attributes, ..
                    }) => {
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
                    Ok(XmlEvent::Characters(text)) => {
                        let map = HashMap::from([(current_element.clone(), text.clone())]);
                        record.load_data(&map)?;
                    }
                    Err(e) => panic!("Error processing event: {}", e),
                    _ => (),
                }
                record.geopoint = Some(geopoint);
                record.activity = Some(activity.clone());
                record.ds = Some(timestamp);
                records.push(record.clone());
            }
            Ok(Some(Workout {
                activity: activity.clone(),
                timestamp,
                records,
            }))
        }
    }
}
