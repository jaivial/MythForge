//! The forge engine: turns a natural-language prompt into modules, entities,
//! fields, views and automations for one company, then serves them through the
//! generic runtime â no restarts.

pub mod catalog;
pub mod generate;
pub mod runtime;
