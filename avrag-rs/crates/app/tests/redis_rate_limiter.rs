use app::adapters::redis_rate_limiter::RedisFixedWindowRateLimiter;
use uuid::Uuid;

#[tokio::test]
async fn redis_fixed_window_limiter_blocks_after_limit() {
    let redis_url =
        std::env::var("TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let limiter = RedisFixedWindowRateLimiter::new(redis_url, 2).await.unwrap();
    let key = format!("org-1:user-1:{}", Uuid::new_v4());

    assert!(limiter.check(&key).await.unwrap().allowed);
    assert!(limiter.check(&key).await.unwrap().allowed);
    assert!(!limiter.check(&key).await.unwrap().allowed);
}
