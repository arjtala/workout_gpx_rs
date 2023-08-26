CREATE TABLE IF NOT EXISTS workouts (
  wid integer PRIMARY KEY UNIQUE NOT NULL,
  activity text,
  ds integer,
  record_locations text
);


