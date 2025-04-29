use http::header::AsHeaderName;

pub(crate) trait ValueGetter {
    /// Get string value by key, or return default if not found or invalid
    fn get_str(&self, key: impl AsHeaderName) -> Option<String>;

    /// Get i32 value by key, or return None if not found or invalid
    fn get_i32(&self, key: impl AsHeaderName) -> Option<i32>;

    /// Get string value with a default fallback
    fn get_str_or_default(&self, key: impl AsHeaderName, default: impl AsRef<str>) -> String {
        self.get_str(key)
            .unwrap_or_else(|| default.as_ref().to_string())
    }

    /// Get i32 value with a default fallback
    fn get_i32_or_default(&self, key: impl AsHeaderName, default: i32) -> i32 {
        self.get_i32(key).unwrap_or(default)
    }
}

impl ValueGetter for http::HeaderMap<http::HeaderValue> {
    fn get_str(&self, key: impl AsHeaderName) -> Option<String> {
        self.get(key)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string())
    }
    fn get_i32(&self, key: impl AsHeaderName) -> Option<i32> {
        self.get(key)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<i32>().ok())
    }
}

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

pub(crate) fn user_agent() -> String {
    format!("aliyun-log-rust-sdk/{}", VERSION)
}

/// Check if an Option<String> is None or contains an empty string
pub(crate) fn is_empty_or_none(option: &Option<String>) -> bool {
    option.as_ref().is_none_or(String::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_user_agent() {
        assert_eq!(user_agent(), format!("aliyun-log-rust-sdk/{}", VERSION));
    }
}
