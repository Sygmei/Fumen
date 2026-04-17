ALTER TABLE score_annotations
    ADD COLUMN IF NOT EXISTS system_y_ratio DOUBLE PRECISION;