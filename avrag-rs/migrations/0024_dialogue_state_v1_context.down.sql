ALTER TABLE dialogue_states
    DROP COLUMN IF EXISTS unresolved_question,
    DROP COLUMN IF EXISTS last_entity,
    DROP COLUMN IF EXISTS last_document;
