-- Challenge-response table for UI perturbation
CREATE TABLE IF NOT EXISTS challenges (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id   TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ NOT NULL,
    solved_at    TIMESTAMPTZ,
    target_x     DOUBLE PRECISION NOT NULL,
    target_y     DOUBLE PRECISION NOT NULL
);

CREATE INDEX IF NOT EXISTS challenges_session_idx ON challenges(session_id);
CREATE INDEX IF NOT EXISTS challenges_expires_idx ON challenges(expires_at);
