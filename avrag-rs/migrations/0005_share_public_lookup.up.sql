CREATE POLICY public_share_token_lookup ON share_tokens
    USING (token = nullif(current_setting('app.public_share_token', true), ''));
