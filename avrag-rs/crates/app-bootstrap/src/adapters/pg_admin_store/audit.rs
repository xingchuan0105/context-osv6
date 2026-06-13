    async fn list_audit_logs(
        &self,
        auth: &AuthContext,
        query: &AdminAuditLogQuery,
    ) -> Result<AdminAuditLogPage, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let total = Self::audit_log_total(tx.as_mut(), query).await?;
        let rows = Self::audit_log_rows(tx.as_mut(), query).await?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(AdminAuditLogPage {
            items: rows
                .into_iter()
                .map(Self::map_audit_log_entry)
                .collect::<Result<Vec<_>, _>>()?,
            total,
            page: query.page.max(1),
            per_page: admin_clamp_audit_per_page(query.per_page),
        })
    }

    async fn export_audit_logs_csv(
        &self,
        auth: &AuthContext,
        query: &AdminAuditLogQuery,
    ) -> Result<String, AppError> {
        let mut tx = self.begin_admin_tx(auth).await?;
        let export_query = AdminAuditLogQuery {
            query: query.query.clone(),
            action: query.action.clone(),
            resource_type: query.resource_type.clone(),
            actor: query.actor.clone(),
            window: query.window.clone(),
            page: 1,
            per_page: 5_000,
        };
        let rows = Self::audit_log_rows(tx.as_mut(), &export_query).await?;
        tx.commit()
            .await
            .map_err(db_err)?;
        Ok(admin_audit_logs_to_csv(
            &rows
                .into_iter()
                .map(Self::map_audit_log_entry)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
