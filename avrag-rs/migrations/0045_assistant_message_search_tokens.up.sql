-- Populate search_tokens for assistant chat messages so global session search can FTS assistant replies.

UPDATE chat_messages
SET search_tokens = trim(coalesce(content, ''))
WHERE role = 'assistant'
  AND (search_tokens IS NULL OR search_tokens = '');
