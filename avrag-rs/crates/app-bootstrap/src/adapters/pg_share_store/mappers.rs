fn map_member(row: sqlx::postgres::PgRow) -> Result<ShareWorkspaceMember, AppError> {
    Ok(ShareWorkspaceMember {
        id: row
            .try_get::<Uuid, _>("id")
            .map(|id| id.to_string())
            .map_err(|error| AppError::internal(error.to_string()))?,
        workspace_id: row
            .try_get::<Uuid, _>("workspace_id")
            .map(|id| id.to_string())
            .map_err(|error| AppError::internal(error.to_string()))?,
        user_id: row
            .try_get::<Option<Uuid>, _>("user_id")
            .ok()
            .flatten()
            .map(|id| id.to_string()),
        email: row.try_get::<Option<String>, _>("email").ok().flatten(),
        access_level: row
            .try_get::<String, _>("access_level")
            .map(|role| ShareAccessLevel::from_role(&role))
            .unwrap_or(ShareAccessLevel::None),
        invite_status: row
            .try_get::<String, _>("invite_status")
            .unwrap_or_else(|_| "accepted".to_string()),
        invited_by: row
            .try_get::<Option<Uuid>, _>("invited_by")
            .ok()
            .flatten()
            .map(|id| id.to_string()),
        invited_at: row
            .try_get::<DateTime<Utc>, _>("invited_at")
            .map(|dt| dt.timestamp())
            .unwrap_or_default(),
        accepted_at: row
            .try_get::<Option<DateTime<Utc>>, _>("accepted_at")
            .ok()
            .flatten()
            .map(|dt| dt.timestamp()),
    })
}
