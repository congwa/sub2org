-- sub2org M2 schema

CREATE TABLE IF NOT EXISTS api_keys (
    id          TEXT PRIMARY KEY,
    key_hash    TEXT NOT NULL UNIQUE,
    owner       TEXT NOT NULL DEFAULT '',
    created_at  BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id             TEXT PRIMARY KEY,
    api_key_id             TEXT NOT NULL,
    worker_id              TEXT NOT NULL,
    provider               TEXT NOT NULL,
    native_conversation_id TEXT,
    created_at             BIGINT NOT NULL,
    last_used_at           BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_api_key ON sessions (api_key_id);
CREATE INDEX IF NOT EXISTS idx_sessions_worker ON sessions (worker_id);
