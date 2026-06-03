ALTER TABLE session_scores ADD COLUMN IF NOT EXISTS confirmed_fraud BOOLEAN DEFAULT NULL;

CREATE TABLE IF NOT EXISTS honeypot_triggers (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id   TEXT NOT NULL,
    visitor_id   TEXT,
    ip           TEXT,
    user_agent   TEXT,
    triggered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS honeypot_session_idx ON honeypot_triggers(session_id);
CREATE INDEX IF NOT EXISTS honeypot_triggered_at_idx ON honeypot_triggers(triggered_at DESC);
