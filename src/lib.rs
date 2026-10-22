//! Typed JSON API for the FFXIV Lodestone.
//!
//! Ruststone fetches Lodestone HTML, parses it with the vendored
//! lodestone-css-selectors, and serves JSON. It has no database. Invalid
//! path and query values are rejected before any upstream request.
//!
//! # Model
//!
//! * [`lodestone::Page`] is one Lodestone page: its URL, request type, and parser.
//! * [`lodestone::Cached`] is that page as JSON. A cache hit is written out as stored.
//! * [`lodestone::ids`] are the values that may appear in a Lodestone URL.
//! * [`settings::Settings`] is the `settings:` block. Unknown keys fail at boot.
//!
//! Display strings scraped from the page (names, ranks, bios) stay strings,
//! because the regional site localizes them. Closed, language-independent
//! sets — jobs, gender symbols, free company focus — are enums.
//!
//! The HTTP routes, status codes, and MCP tools are documented in the crate README.

pub mod app;
pub mod controllers;
pub mod lodestone;
pub mod mcp;
pub mod settings;
