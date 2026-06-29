-- Interim plain-text backfill for assistant messages (jieba resegment runs post-migrate in Rust).

UPDATE chat_messages
SET search_tokens = trim(coalesce(content, ''))
WHERE role = 'assistant'
  AND (search_tokens IS NULL OR search_tokens = '');
