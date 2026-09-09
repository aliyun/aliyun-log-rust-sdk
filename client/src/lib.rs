//! Asynchronous client for Alibaba Cloud Simple Log Service (SLS).
//!
//! Create a [`Config`] and pass it to [`Client::from_config`] using [`FromConfig`].
//! Requests are asynchronous and use the Tokio runtime.
//!
//! # Credentials Providers
//!
//! Use a creation helper with [`ConfigBuilder::credentials_provider`]:
//!
//! * [`ecs_ram_role_credentials_provider`] obtains temporary credentials for an
//!   explicitly named ECS RAM role. See the helper's prerequisites and example.
//! * [`static_credentials_provider`] configures fixed access keys and an optional STS token.
//! * [`environment_credentials_provider`] reads access keys and an optional STS token
//!   from configurable environment variables.
//! * Implement [`CredentialsProvider`] to connect another credentials source.
//!
//! The SDK manages credential refreshes. The existing [`ConfigBuilder::access_key`]
//! and [`ConfigBuilder::sts`] methods remain available for fixed credentials.

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
    ecs_ram_role_credentials_provider, environment_credentials_provider,
    environment_credentials_provider_builder, static_credentials_provider, Credentials,
    CredentialsError, CredentialsProvider, EcsRamRoleCredentialsProvider,
    EnvironmentCredentialsProvider, EnvironmentCredentialsProviderBuilder,
    SharedCredentialsProvider, StaticCredentialsProvider,
};
mod macros;
mod request;
mod response;

// Compile the guides' examples as doctests, without adding public API items.
#[cfg(doctest)]
#[doc = include_str!("../docs/credentials.md")]
mod credentials_guide {}

#[cfg(doctest)]
#[doc = include_str!("../docs/credentials_cn.md")]
mod credentials_guide_cn {}
