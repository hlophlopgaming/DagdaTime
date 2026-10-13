CREATE TABLE events (
    id        INTEGER PRIMARY KEY,
    bucket    TEXT NOT NULL CHECK (length(bucket) > 0),
    start_ts  INTEGER NOT NULL CHECK (start_ts >= 0),
    end_ts    INTEGER NOT NULL CHECK (end_ts >= start_ts),
    data      TEXT NOT NULL CHECK (json_valid(data))
);
CREATE INDEX idx_events_bucket_start ON events(bucket, start_ts);

CREATE TABLE tags (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    color     TEXT,
    parent_id INTEGER REFERENCES tags(id)
);

CREATE TABLE tag_ranges (
    id       INTEGER PRIMARY KEY,
    tag_id   INTEGER NOT NULL REFERENCES tags(id),
    start_ts INTEGER NOT NULL CHECK (start_ts >= 0),
    end_ts   INTEGER NOT NULL CHECK (end_ts >= start_ts),
    note     TEXT
);

CREATE TABLE rules (
    id       INTEGER PRIMARY KEY,
    field    TEXT NOT NULL CHECK (field IN ('app', 'title', 'url')),
    pattern  TEXT NOT NULL,
    tag_id   INTEGER NOT NULL REFERENCES tags(id),
    priority INTEGER NOT NULL DEFAULT 0
);
