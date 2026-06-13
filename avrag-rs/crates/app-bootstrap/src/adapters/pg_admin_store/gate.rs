    async fn ensure_admin_access(&self, auth: &AuthContext) -> Result<(), AppError> {
        let tx = self.begin_admin_tx(auth).await?;
        tx.commit().await.map_err(db_err)
    }

