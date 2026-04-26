ALTER TABLE dialogue_states
    ADD COLUMN IF NOT EXISTS last_document TEXT,
    ADD COLUMN IF NOT EXISTS last_entity TEXT,
    ADD COLUMN IF NOT EXISTS unresolved_question TEXT;
