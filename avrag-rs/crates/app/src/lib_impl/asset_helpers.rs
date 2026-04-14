fn build_docscope_metadata(metadata: Vec<common::SummaryMetadata>) -> common::DocScopeMetadata {
    let mut languages = Vec::new();
    let mut domains = Vec::new();
    let mut genres = Vec::new();
    let mut eras = Vec::new();

    for meta in &metadata {
        if !meta.language.is_empty() && meta.language != "unknown" {
            languages.push(meta.language.clone());
        }
        if !meta.domain.is_empty() && meta.domain != "unknown" {
            domains.push(meta.domain.clone());
        }
        if !meta.genre.is_empty() && meta.genre != "unknown" {
            genres.push(meta.genre.clone());
        }
        if !meta.era.is_empty() && meta.era != "unknown" {
            eras.push(meta.era.clone());
        }
    }

    languages.sort();
    languages.dedup();
    domains.sort();
    domains.dedup();
    genres.sort();
    genres.dedup();
    eras.sort();
    eras.dedup();

    common::DocScopeMetadata {
        documents: metadata,
        profile: common::DocScopeProfile {
            languages,
            domains,
            genres,
            eras,
        },
    }
}

impl AppState {
    async fn resolve_citation_asset_url(&self, asset: &DocumentAssetRow) -> Option<String> {
        let storage_path = asset.storage_path.as_deref()?;
        if is_remote_asset_reference(storage_path) {
            return Some(storage_path.to_string());
        }

        match self
            .object_store
            .presigned_get_url(
                storage_path,
                self.config.object_storage.download_url_expire_sec,
            )
            .await
        {
            Ok(url) if !url.starts_with("file://") => Some(url),
            _ => Some(format!("/api/v1/chat/citations/assets/{}", asset.asset_id)),
        }
    }
}

fn build_redis_url(addr: &str, password: &str, db: i64) -> String {
    if password.is_empty() {
        format!("redis://{addr}/{db}")
    } else {
        format!("redis://:{password}@{addr}/{db}")
    }
}

fn is_remote_asset_reference(value: &str) -> bool {
    common::is_remote_url(value)
}

fn infer_mime_type_from_path(path: &str) -> Option<String> {
    common::infer_mime_type(path).map(|s| s.to_string())
}
