ALTER TABLE documents
    DROP COLUMN IF EXISTS upload_validation_error,
    DROP COLUMN IF EXISTS upload_validated_at,
    DROP COLUMN IF EXISTS upload_sha256,
    DROP COLUMN IF EXISTS upload_size_bytes;
