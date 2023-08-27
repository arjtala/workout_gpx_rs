CREATE TABLE IF NOT EXISTS workouts_records (
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

CREATE TABLE IF NOT EXISTS workouts (
  wid integer PRIMARY KEY UNIQUE NOT NULL,
  activity text,
  ds integer,
  record_locations text
);

CREATE VIRTUAL TABLE workouts_geo
USING geopoly ();

