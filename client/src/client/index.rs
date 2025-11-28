use serde::{Deserialize, Serialize};

mod create_index;
pub use create_index::*;

mod update_index;
pub use update_index::*;

mod get_index;
pub use get_index::*;

mod delete_index;
pub use delete_index::*;

use super::*;

/// Builder for constructing Index configuration.
///
/// Provides a fluent API for building complex index configurations with full-text search,
/// field indexing, and log clustering capabilities.
///
/// # Examples
///
/// ```no_run
/// use aliyun_log_rust_sdk::{Index, FullTextIndex, FieldIndex, IndexKeyText, token_list};
/// use std::collections::HashMap;
///
/// // Configure with full-text index
/// let full_text_index = FullTextIndex {
///     case_sensitive: false,
///     chn: true,
///     token: token_list![",", " ", ";"],
/// };
///
/// let index = Index::builder()
///     .line(full_text_index)
///     .build();
///
/// // Configure with field indexes
/// let mut keys = HashMap::new();
/// keys.insert(
///     "level".to_string(),
///     FieldIndex::Text(IndexKeyText {
///         case_sensitive: false,
///         alias: None,
///         chn: false,
///         token: token_list![],
///         doc_value: true,
///     })
/// );
///
/// let index = Index::builder()
///     .keys(keys)
///     .build();
/// ```
#[derive(Default)]
pub struct IndexBuilder {
    max_text_len: Option<i32>,
    line: Option<FullTextIndex>,
    keys: Option<std::collections::HashMap<String, FieldIndex>>,
    scan_index: Option<bool>,
    log_reduce: Option<bool>,
    log_reduce_white_list: Option<Vec<String>>,
    log_reduce_black_list: Option<Vec<String>>,
}

impl IndexBuilder {
    pub fn new() -> Self {
        Self {
            max_text_len: None,
            line: None,
            keys: None,
            scan_index: None,
            log_reduce: None,
            log_reduce_white_list: None,
            log_reduce_black_list: None,
        }
    }

    /// Set the maximum length for statistics fields.
    ///
    /// # Arguments
    ///
    /// * `max_text_len` - Maximum text length for indexed fields’ doc value
    pub fn max_text_len(mut self, max_text_len: i32) -> Self {
        self.max_text_len = Some(max_text_len);
        self
    }

    /// Set full-text index configuration.
    ///
    /// # Arguments
    ///
    /// * `line` - Full-text index configuration
    pub fn line(mut self, line: FullTextIndex) -> Self {
        self.line = Some(line);
        self
    }

    /// Set field index configuration.
    ///
    /// # Arguments
    ///
    /// * `keys` - Field index configuration map
    pub fn keys(mut self, keys: std::collections::HashMap<String, FieldIndex>) -> Self {
        self.keys = Some(keys);
        self
    }

    /// Enable or disable scan index.
    ///
    /// # Arguments
    ///
    /// * `scan_index` - Whether to enable scan index
    pub fn scan_index(mut self, scan_index: bool) -> Self {
        self.scan_index = Some(scan_index);
        self
    }

    /// Enable or disable log clustering.
    ///
    /// # Arguments
    ///
    /// * `log_reduce` - Whether to enable log clustering
    pub fn log_reduce(mut self, log_reduce: bool) -> Self {
        self.log_reduce = Some(log_reduce);
        self
    }

    /// Set whitelist for log clustering fields.
    ///
    /// Only effective when log clustering is enabled.
    ///
    /// # Arguments
    ///
    /// * `log_reduce_white_list` - List of field names to include in clustering
    pub fn log_reduce_white_list(mut self, log_reduce_white_list: Vec<String>) -> Self {
        self.log_reduce_white_list = Some(log_reduce_white_list);
        self
    }

    /// Set blacklist for log clustering fields.
    ///
    /// Only effective when log clustering is enabled.
    ///
    /// # Arguments
    ///
    /// * `log_reduce_black_list` - List of field names to exclude from clustering
    pub fn log_reduce_black_list(mut self, log_reduce_black_list: Vec<String>) -> Self {
        self.log_reduce_black_list = Some(log_reduce_black_list);
        self
    }

    /// Build the Index configuration.
    pub fn build(self) -> Index {
        Index {
            max_text_len: self.max_text_len,
            line: self.line,
            keys: self.keys,
            scan_index: self.scan_index,
            log_reduce: self.log_reduce,
            log_reduce_white_list: self.log_reduce_white_list,
            log_reduce_black_list: self.log_reduce_black_list,
        }
    }
}

/// Index configuration for a logstore.
///
/// Defines how logs are indexed for querying and analysis, including full-text search,
/// field indexing, and log clustering capabilities.
///
/// # Examples
///
/// ```no_run
/// use aliyun_log_rust_sdk::{Index, FullTextIndex, FieldIndex, IndexKeyText, token_list};
/// use std::collections::HashMap;
///
/// // Configure with full-text index
/// let full_text_index = FullTextIndex {
///     case_sensitive: false,
///     chn: true,
///     token: token_list![",", " ", ";"],
/// };
///
/// let index = Index::builder()
///     .line(full_text_index)
///     .build();
///
/// // Configure with field indexes
/// let mut keys = HashMap::new();
/// keys.insert(
///     "level".to_string(),
///     FieldIndex::Text(IndexKeyText {
///         case_sensitive: false,
///         alias: None,
///         chn: false,
///         token: token_list![],
///         doc_value: true,
///     })
/// );
///
/// let index = Index::builder()
///     .keys(keys)
///     .build();
/// ```
#[derive(Serialize, Deserialize, Default)]
pub struct Index {
    /// Maximum length for statistics fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_text_len: Option<i32>,
    /// Full-text index configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<FullTextIndex>,
    /// Field index configuration map
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<std::collections::HashMap<String, FieldIndex>>,
    /// Whether to enable scan index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_index: Option<bool>,
    /// Whether to enable log clustering
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_reduce: Option<bool>,
    /// Whitelist for log clustering fields (only effective when log_reduce is enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_reduce_white_list: Option<Vec<String>>,
    /// Blacklist for log clustering fields (only effective when log_reduce is enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_reduce_black_list: Option<Vec<String>>,
}

impl Index {
    /// Create a new empty Index configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new IndexBuilder for fluent configuration.
    pub fn builder() -> IndexBuilder {
        IndexBuilder::new()
    }
}

/// Full-text index configuration.
///
/// Configures how full-text search is performed on log content.
///
/// # Examples
///
/// ```
/// use aliyun_log_rust_sdk::{FullTextIndex, token_list};
///
/// let full_text_index = FullTextIndex {
///     case_sensitive: false,
///     chn: true,
///     token: token_list![",", " ", ";", "\n", "\t"],
/// };
/// ```
#[derive(Serialize, Deserialize)]
pub struct FullTextIndex {
    /// Whether the search is case-sensitive
    #[serde(rename = "caseSensitive")]
    pub case_sensitive: bool,
    /// Whether to enable Chinese word segmentation
    pub chn: bool,
    /// List of delimiter tokens for tokenization
    pub token: Vec<String>,
}

/// Field index type enumeration.
///
/// Defines different types of field indexes that can be applied to log fields.
///
/// # Variants
///
/// * `Text` - Text field index for string content
/// * `Long` - Long integer field index for numeric values
/// * `Double` - Double precision float field index for decimal numbers
/// * `Json` - JSON field index for nested JSON content
///
/// # Examples
///
/// ```
/// use aliyun_log_rust_sdk::{FieldIndex, IndexKeyText, token_list};
///
/// let field_index = FieldIndex::Text(IndexKeyText {
///     case_sensitive: false,
///     alias: None,
///     chn: false,
///     token: token_list![",", "\t", " "],
///     doc_value: true,
/// });
/// ```
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldIndex {
    Text(IndexKeyText),
    Long(IndexKeyLong),
    Double(IndexKeyDouble),
    Json(IndexKeyJson),
}

/// JSON field index configuration.
///
/// Configures indexing for JSON-type fields with nested structure support.
///
/// # Examples
///
/// ```
/// use aliyun_log_rust_sdk::{IndexKeyJson, token_list};
/// use std::collections::HashMap;
///
/// let json_index = IndexKeyJson {
///     case_sensitive: false,
///     alias: None,
///     chn: false,
///     token: token_list![",", "\t", " "],
///     doc_value: true,
///     max_depth: 3,
///     index_all: true,
///     json_keys: None,
/// };
/// ```
#[derive(Serialize, Deserialize)]
pub struct IndexKeyJson {
    /// Whether the search is case-sensitive
    #[serde(rename = "caseSensitive")]
    pub case_sensitive: bool,
    /// Field alias for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Whether to enable Chinese word segmentation
    pub chn: bool,
    /// List of delimiter tokens for tokenization
    pub token: Vec<String>,
    /// Whether to enable doc value for analytics
    pub doc_value: bool,
    /// Maximum depth for JSON structure indexing, -1 means no limit
    pub max_depth: i32,
    /// Whether to index all fields in the JSON structure
    pub index_all: bool,
    /// Specific JSON keys to index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_keys: Option<std::collections::HashMap<String, IndexJsonKey>>,
}

/// JSON nested field index type enumeration.
///
/// Defines the types of indexes that can be applied to nested fields within JSON.
///
/// # Examples
///
/// ```
/// use aliyun_log_rust_sdk::{IndexJsonKey, IndexKeyText, token_list};
///
/// let json_field = IndexJsonKey::Text(IndexKeyText {
///     case_sensitive: false,
///     alias: None,
///     chn: false,
///     token: token_list![",", "\t", " "],
///     doc_value: true,
/// });
/// ```
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IndexJsonKey {
    Text(IndexKeyText),
    Long(IndexKeyLong),
    Double(IndexKeyDouble),
}

/// Text field index configuration.
///
/// Configures indexing for text-type fields with full-text search capabilities.
#[derive(Serialize, Deserialize)]
pub struct IndexKeyText {
    /// Whether the search is case-sensitive
    #[serde(rename = "caseSensitive")]
    pub case_sensitive: bool,
    /// Field alias for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Whether to enable Chinese word segmentation
    pub chn: bool,
    /// List of delimiter tokens for tokenization
    pub token: Vec<String>,
    /// Whether to enable doc value for analytics
    pub doc_value: bool,
}

/// Long integer field index configuration.
///
/// Configures indexing for long integer fields.
#[derive(Serialize, Deserialize)]
pub struct IndexKeyLong {
    /// Field alias for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Whether to enable doc value for analytics
    pub doc_value: bool,
}

/// Double precision float field index configuration.
///
/// Configures indexing for double precision floating-point fields.
#[derive(Serialize, Deserialize)]
pub struct IndexKeyDouble {
    /// Field alias for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Whether to enable doc value for analytics
    pub doc_value: bool,
}

/// Text field index configuration.
///
/// Configures indexing for text-type fields with full-text search capabilities.
#[derive(Serialize, Deserialize)]
pub struct IndexKeyJsonText {
    /// Field alias for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Whether to enable doc value for analytics
    pub doc_value: bool,
}
