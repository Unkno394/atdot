CREATE TABLE IF NOT EXISTS pattern_candidates (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sequence         JSONB  NOT NULL,
    status           TEXT   NOT NULL DEFAULT 'pending',
    contributing     JSONB  NOT NULL DEFAULT '[]',
    humanity_scores  JSONB  NOT NULL DEFAULT '[]',
    timestamps       JSONB  NOT NULL DEFAULT '[]',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    quarantine_until TIMESTAMPTZ,
    humanity_avg     DOUBLE PRECISION,
    temporal_entropy DOUBLE PRECISION
);

CREATE TABLE IF NOT EXISTS session_scores (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id  TEXT  NOT NULL,
    api_key_id  UUID  NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
    score       DOUBLE PRECISION NOT NULL,
    l1_score    DOUBLE PRECISION,
    l2_score    DOUBLE PRECISION,
    l3_score    DOUBLE PRECISION,
    action      TEXT  NOT NULL DEFAULT 'allow',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS session_scores_session_id_idx ON session_scores(session_id);
CREATE INDEX IF NOT EXISTS session_scores_created_at_idx ON session_scores(created_at DESC);
CREATE INDEX IF NOT EXISTS pattern_candidates_status_idx  ON pattern_candidates(status);
