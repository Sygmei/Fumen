ALTER TABLE score_annotations
    ADD COLUMN IF NOT EXISTS instrument_name TEXT;
