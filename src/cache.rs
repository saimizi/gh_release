use crate::errors::Result;
use jlogger_tracing::jdebug;
use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tokio::fs;

/// Cache for GitHub API responses
pub struct Cache {
    cache_dir: PathBuf,
    ttl: Duration,
    enabled: bool,
}

impl Cache {
    /// Create a new cache instance
    pub fn new(enabled: bool) -> Self {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ghr");

        Self {
            cache_dir,
            ttl: Duration::from_secs(24 * 60 * 60), // 24 hours default
            enabled,
        }
    }

    /// Create a cache with custom TTL
    #[allow(dead_code)]
    pub fn with_ttl(enabled: bool, ttl_hours: u64) -> Self {
        let mut cache = Self::new(enabled);
        cache.ttl = Duration::from_secs(ttl_hours * 60 * 60);
        cache
    }

    /// Get cache file path for a given key
    fn cache_path(&self, key: &str) -> PathBuf {
        // Create a safe filename from the key
        let safe_key = key.replace(['/', ':'], "_");
        self.cache_dir.join(format!("{}.json", safe_key))
    }

    /// Get cached value if it exists and is not expired
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        if !self.enabled {
            return None;
        }

        let path = self.cache_path(key);
        if !path.exists() {
            jdebug!("Cache miss: {}", key);
            return None;
        }

        // Check if expired
        let metadata = fs::metadata(&path).await.ok()?;
        let modified = metadata.modified().ok()?;
        let age = SystemTime::now().duration_since(modified).ok()?;

        if age > self.ttl {
            jdebug!("Cache expired: {}", key);
            // Cleanup expired entry
            let _ = fs::remove_file(&path).await;
            return None;
        }

        // Read and parse cached data
        let data = fs::read_to_string(&path).await.ok()?;
        let result: T = serde_json::from_str(&data).ok()?;

        jdebug!("Cache hit: {} (age: {:?})", key, age);
        Some(result)
    }

    /// Set cached value
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Ensure cache directory exists
        fs::create_dir_all(&self.cache_dir).await?;

        let path = self.cache_path(key);
        let data = serde_json::to_string(value)?;
        fs::write(&path, data).await?;

        jdebug!("Cache set: {}", key);
        Ok(())
    }

    /// Clear all cached entries
    #[allow(dead_code)]
    pub async fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).await?;
            jdebug!("Cache cleared");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        value: String,
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let cache = Cache::new(false);
        let data = TestData {
            value: "test".to_string(),
        };

        cache.set("test-key", &data).await.unwrap();
        let result: Option<TestData> = cache.get("test-key").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_set_get() {
        let cache = Cache::new(true);
        let data = TestData {
            value: "test".to_string(),
        };

        cache.set("test-key-2", &data).await.unwrap();
        let result: Option<TestData> = cache.get("test-key-2").await;
        assert_eq!(result, Some(data));

        // Cleanup
        cache.clear().await.unwrap();
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = Cache::new(true);
        let result: Option<TestData> = cache.get("nonexistent-key").await;
        assert!(result.is_none());
    }
}
