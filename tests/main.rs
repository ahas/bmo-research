//! The crate's one test harness (bmo-chat's docs/conventions.md, 「Rust builds」): each file of tests a
//! module of it, sharing the process.

mod support;

mod dependencies;
mod dive;
mod engine;
mod layout;
mod literals;
mod openapi;
mod page;
mod personal_data;
mod pull;
mod reading;
mod review;
mod routes;
mod runner;
mod scan;
mod sources;
mod stack;
mod store;
mod structure;
mod tool;
mod translation;
