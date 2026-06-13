    async fn record_share_product_event(
        &self,
        event: analytics::ProductEvent,
    ) -> Result<(), AppError> {
        let analytics = analytics::AnalyticsService::new(self.repo.raw().clone());
        analytics
            .record_product_event(&event)
            .await
            .map_err(|error| AppError::internal(error.to_string()))
    }
