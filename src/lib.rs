//! DIKT: decisions, ideas, knowledge and tasks as validated local Markdown.

pub mod cli;
pub mod error;
pub mod init;
pub mod store;
pub mod views;
pub mod yaml;

pub use error::{Error, Result};
