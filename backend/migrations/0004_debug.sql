ALTER TABLE session_scores ADD COLUMN IF NOT EXISTS reasons        JSONB            NOT NULL DEFAULT '[]';
ALTER TABLE session_scores ADD COLUMN IF NOT EXISTS embedding_score DOUBLE PRECISION;
ALTER TABLE session_scores ADD COLUMN IF NOT EXISTS event_type      TEXT;
