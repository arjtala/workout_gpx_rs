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

