use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use actix_web::HttpRequest;

use crate::auth::Auth;
use crate::services::error::ServiceError;

#[derive(Clone, Copy)]
struct Bucket {
    count: u32,
    reset_at: Instant,
}

pub struct InMemoryRateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
}

impl InMemoryRateLimiter {
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn check_and_record(
        &self,
        key: &str,
        max_allowed: u32,
        window: Duration,
    ) -> Result<(), ServiceError> {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap_or_else(|p| p.into_inner());

        if map.len() > 10_000 {
            map.retain(|_, b| b.reset_at > now);
        }

        let entry = map.entry(key.to_string()).or_insert(Bucket {
            count: 0,
            reset_at: now + window,
        });

        if now >= entry.reset_at {
            entry.count = 1;
            entry.reset_at = now + window;
            Ok(())
        } else if entry.count >= max_allowed {
            Err(ServiceError::TooManyRequests(
                "too many requests".to_owned(),
            ))
        } else {
            entry.count += 1;
            Ok(())
        }
    }

    pub fn check_login_blocked(&self, key: &str, max_failures: u32) -> Result<(), ServiceError> {
        let now = Instant::now();
        let map = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(bucket) = map.get(key) {
            if now < bucket.reset_at && bucket.count >= max_failures {
                return Err(ServiceError::TooManyRequests(
                    "too many attempts".to_owned(),
                ));
            }
        }
        Ok(())
    }

    pub fn record_login_failure(
        &self,
        key: &str,
        max_failures: u32,
        window: Duration,
    ) -> Result<(), ServiceError> {
        let now = Instant::now();
        let mut map = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        let entry = map.entry(key.to_string()).or_insert(Bucket {
            count: 0,
            reset_at: now + window,
        });

        if now >= entry.reset_at {
            entry.count = 1;
            entry.reset_at = now + window;
            Ok(())
        } else {
            entry.count += 1;
            if entry.count >= max_failures {
                Err(ServiceError::TooManyRequests(
                    "too many attempts".to_owned(),
                ))
            } else {
                Ok(())
            }
        }
    }

    pub fn reset(&self, key: &str) {
        let mut map = self.buckets.lock().unwrap_or_else(|p| p.into_inner());
        map.remove(key);
    }
}

pub static RATE_LIMITER: LazyLock<InMemoryRateLimiter> = LazyLock::new(InMemoryRateLimiter::new);

pub fn extract_client_ip(req: &HttpRequest) -> String {
    req.headers()
        .get("cf-connecting-ip")
        .or_else(|| req.headers().get("x-forwarded-for"))
        .or_else(|| req.headers().get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            req.connection_info()
                .realip_remote_addr()
                .unwrap_or("127.0.0.1")
                .to_owned()
        })
}

pub fn extract_client_fingerprint(req: &HttpRequest) -> String {
    let ip = extract_client_ip(req);
    let ua = req
        .headers()
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");
    format!("{ip}:{ua}")
}

pub fn check_jwt_rate_limit(
    auth: &Auth,
    req: &HttpRequest,
    action: &str,
    max_requests: u32,
    window_secs: u64,
) -> Result<(), ServiceError> {
    let duration = Duration::from_secs(window_secs);
    let uuid = auth.player_uuid();
    let jti = auth.claims.jti.as_deref().unwrap_or("no-jti");
    let fp = extract_client_fingerprint(req);

    let user_key = format!("user:{uuid}:{action}");
    RATE_LIMITER.check_and_record(&user_key, max_requests, duration)?;

    let token_key = format!("token:{jti}:{action}");
    RATE_LIMITER.check_and_record(&token_key, max_requests, duration)?;

    let fp_key = format!("fp:{fp}:{action}");
    RATE_LIMITER.check_and_record(&fp_key, max_requests * 2, duration)?;

    Ok(())
}

pub fn check_ip_rate_limit(
    req: &HttpRequest,
    action: &str,
    max_requests: u32,
    window_secs: u64,
) -> Result<(), ServiceError> {
    let fp = extract_client_fingerprint(req);
    let key = format!("public:{fp}:{action}");
    RATE_LIMITER.check_and_record(&key, max_requests, Duration::from_secs(window_secs))
}
