//! Anthropic client SDK access for MythForge.
//!
//! One choke point (`Ai::from_config`) builds the client, exactly like
//! `claude_session.sage_client_route()` did in MythAgent: the base URL is
//! configurable so the same code talks to api.anthropic.com or to an
//! Anthropic-compatible gateway (z.ai GLM, MiniMax, ...).

pub mod agent;
pub mod client;
