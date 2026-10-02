//! Multi-tenant PostgreSQL: one database per company + a control-plane DB.
//!
//! * `forge`        â control plane (users, companies, memberships, mascots, agents,
//!                     tools, automations, chats, google tokens, audit).
//! * `co_<slug>`    â one database per company, provisioned at runtime by
//!                     [`tenant::provision`], never by hand and never with a restart.

pub mod schema;
pub mod tenant;
