-- Boulevard D1 schema (SQLite). Times are Unix milliseconds.
-- `events` is append-only: triggers below refuse UPDATE and DELETE.

CREATE TABLE users (
  id             TEXT PRIMARY KEY,
  email          TEXT NOT NULL UNIQUE,
  email_hash     TEXT NOT NULL,
  status         TEXT NOT NULL DEFAULT 'active',  -- active | suspended | banned | deleted
  status_reason  TEXT,
  created_at     INTEGER NOT NULL,
  verified_at    INTEGER,
  last_active_at INTEGER NOT NULL,
  plus_active    INTEGER NOT NULL DEFAULT 0,
  plus_source    TEXT,
  is_admin       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX users_status_idx ON users(status);
CREATE INDEX users_email_hash_idx ON users(email_hash);

CREATE TABLE profiles (
  user_id      TEXT PRIMARY KEY REFERENCES users(id),
  display_name TEXT NOT NULL,
  gender       TEXT NOT NULL,
  year         TEXT NOT NULL,
  major        TEXT NOT NULL,
  greek        TEXT NOT NULL DEFAULT '',
  intent       TEXT NOT NULL,
  looking_for  TEXT NOT NULL,
  bio          TEXT NOT NULL,
  completed    INTEGER NOT NULL DEFAULT 1,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE user_filters (
  user_id    TEXT PRIMARY KEY REFERENCES users(id),
  intent     TEXT,
  year       TEXT,
  greek      TEXT,   -- Boulevard+ only
  major      TEXT,   -- Boulevard+ only
  updated_at INTEGER NOT NULL
);

CREATE TABLE otp_codes (
  id          TEXT PRIMARY KEY,
  email       TEXT NOT NULL,
  code_hash   TEXT NOT NULL,
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER NOT NULL,
  attempts    INTEGER NOT NULL DEFAULT 0,
  consumed_at INTEGER,
  ip          TEXT
);
CREATE INDEX otp_email_idx ON otp_codes(email, created_at);

CREATE TABLE sessions (
  id           TEXT PRIMARY KEY,   -- sha256 of the bearer token
  user_id      TEXT NOT NULL REFERENCES users(id),
  created_at   INTEGER NOT NULL,
  expires_at   INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  revoked_at   INTEGER,
  user_agent   TEXT
);
CREATE INDEX sessions_user_idx ON sessions(user_id);

CREATE TABLE swipes (
  user_id    TEXT NOT NULL,
  target_id  TEXT NOT NULL,          -- a user id or a stand-in id
  direction  TEXT NOT NULL,          -- like | pass
  note       TEXT,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (user_id, target_id)
);
CREATE INDEX swipes_target_idx ON swipes(target_id, direction);
CREATE INDEX swipes_user_time_idx ON swipes(user_id, direction, created_at);

CREATE TABLE matches (
  id                   TEXT PRIMARY KEY,
  user_a               TEXT NOT NULL,   -- lexically smaller id
  user_b               TEXT NOT NULL,
  created_at           INTEGER NOT NULL,
  closed_at            INTEGER,
  closed_reason        TEXT,            -- unmatched | blocked | deleted | moderation
  closed_by            TEXT,
  last_message_at      INTEGER,
  last_message_preview TEXT,
  last_message_from    TEXT
);
-- One open match per pair; a closed match may be followed by a new one.
CREATE UNIQUE INDEX matches_open_pair_idx ON matches(user_a, user_b) WHERE closed_at IS NULL;
CREATE INDEX matches_a_idx ON matches(user_a, closed_at);
CREATE INDEX matches_b_idx ON matches(user_b, closed_at);

CREATE TABLE blocks (
  blocker_id TEXT NOT NULL,
  blocked_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  PRIMARY KEY (blocker_id, blocked_id)
);
CREATE INDEX blocks_blocked_idx ON blocks(blocked_id);

CREATE TABLE reports (
  id              TEXT PRIMARY KEY,
  reporter_id     TEXT NOT NULL,
  target_id       TEXT NOT NULL,
  match_id        TEXT,
  reason          TEXT NOT NULL,     -- fake_profile | harassment | inappropriate_content | underage | spam | other
  details         TEXT,
  status          TEXT NOT NULL DEFAULT 'open',   -- open | resolved
  created_at      INTEGER NOT NULL,
  resolved_at     INTEGER,
  resolved_by     TEXT,
  resolution      TEXT,              -- dismiss | warn | suspend | ban
  resolution_note TEXT
);
CREATE INDEX reports_status_idx ON reports(status, created_at);
CREATE INDEX reports_target_idx ON reports(target_id);

CREATE TABLE photos (
  id           TEXT PRIMARY KEY,
  user_id      TEXT NOT NULL REFERENCES users(id),
  position     INTEGER NOT NULL,
  r2_key       TEXT NOT NULL,
  content_type TEXT NOT NULL,
  bytes        INTEGER NOT NULL,
  created_at   INTEGER NOT NULL
);
CREATE INDEX photos_user_idx ON photos(user_id, position);

CREATE TABLE events (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  ts         INTEGER NOT NULL,
  type       TEXT NOT NULL,
  actor_id   TEXT,
  subject_id TEXT,
  payload    TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX events_actor_idx ON events(actor_id, id);
CREATE INDEX events_subject_idx ON events(subject_id, id);
CREATE INDEX events_type_idx ON events(type, id);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events
BEGIN SELECT RAISE(ABORT, 'events are append-only'); END;

CREATE TABLE rate_limits (
  key          TEXT PRIMARY KEY,
  window_start INTEGER NOT NULL,
  count        INTEGER NOT NULL
);
