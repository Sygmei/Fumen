ALTER TABLE processing_jobs
    ADD COLUMN IF NOT EXISTS progress_json TEXT;
