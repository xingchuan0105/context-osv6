CREATE TABLE message_tags (
    id BIGSERIAL PRIMARY KEY,
    message_id BIGINT NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(message_id, tag)
);

CREATE INDEX idx_message_tags_tag ON message_tags(tag);
CREATE INDEX idx_message_tags_message_id ON message_tags(message_id);
