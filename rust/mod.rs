//! TikTok Creator Studio module.
//!
//! Provides video items management, watermark-free playback/download link extraction,
//! studio profile inspection, and video publishing workflows within the gateway platform.
//!
//! Structured according to Clean Architecture & Domain-Driven Design (DDD):
//! - `types`: Domain entities, value objects, and domain invariants.
//! - `ports`: Outbound port traits (`TikTokClientPort`).
//! - `service`: Application use cases (`TikTokStudioService`).
//! - `client`: Outbound `ureq` transport adapter (`UreqTikTokClient`).
//! - `protocol`: Wire JSON DTO parsers and request payload builders.
//! - `adapter`: Inbound HTTP route controller (`dispatch_builtin_tiktok`).
//! - `handlers`: Outbound HTTP response presenters.

pub mod adapter;
pub mod client;
pub mod config;
pub mod handlers;
pub mod ports;
pub mod protocol;
pub mod service;
pub mod types;

#[cfg(test)]
mod tests;

pub use adapter::*;
pub use client::*;
pub use config::*;
pub use handlers::*;
pub use ports::*;
pub use protocol::*;
pub use service::*;
pub use types::*;
