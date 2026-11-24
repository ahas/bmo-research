//! The crate's one test harness (bmo-chat's docs/conventions.md, 「Rust builds」): each file of tests a
//! module of it, sharing the process.

mod support;

mod baseline;
mod dependencies;
mod dive;
mod engine;
mod layout;
mod literals;
mod openapi;
mod page;
mod pull;
mod reading;
mod references;
mod review;
mod routes;
mod runner;
mod scan;
mod sources;
mod stack;
mod stop;
mod store;
mod structure;
mod tool;
mod translation;
