-- Section overview text from joint profile+summary extraction (no chunk_id binding).
ALTER TABLE document_toc
    ADD COLUMN IF NOT EXISTS overview TEXT;
