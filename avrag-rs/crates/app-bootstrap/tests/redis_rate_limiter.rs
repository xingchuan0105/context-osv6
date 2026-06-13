use app_bootstrap::{RedisFixedWindowRateLimiter, build_rate_limit_backend};
use uuid::Uuid;

#[test]
fn build_rate_limit_backend_returns_none_for_empty_url() {
    assert!(build_rate_limit_backend("").is_none());
    assert!(build_rate_limit_backend("   ").is_none());
}

#[tokio::test]
async fn redis_fixed_window_limiter_blocks_after_limit() {
    let redis_url = match std::env::var("TEST_REDIS_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => {
            eprintln!("skip redis_fixed_window_limiter_blocks_after_limit: TEST_REDIS_URL not set");
            return;
        }
    };

    let limiter = match RedisFixedWindowRateLimiter::new(redis_url, 2).await {
        Ok(limiter) => limiter,
        Err(err) => {
            eprintln!("skip redis_fixed_window_limiter_blocks_after_limit: {err}");
            return;
        }
    };
    let key = format!("org-1:user-1:{}", Uuid::new_v4());

    assert!(limiter.check(&key).await.unwrap().allowed);
    assert!(limiter.check(&key).await.unwrap().allowed);
    assert!(!limiter.check(&key).await.unwrap().allowed);
}
