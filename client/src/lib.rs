mod client;
mod common;
mod compress;
mod config;
mod credentials;
mod error;
mod utils;

pub use self::error::*;
/// Attribute for implementing [`CredentialsProvider`] with an async method.
pub use async_trait::async_trait;
pub use client::*;
pub use config::{Config, ConfigBuilder};
pub use credentials::{
    static_credentials_provider, Credentials, CredentialsError, CredentialsProvider,
    SharedCredentialsProvider, StaticCredentialsProvider,
};
mod macros;
mod request;
mod response;
