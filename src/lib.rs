// Model
pub mod model;

// Generic
mod whitespace;
mod words;
mod quoted;
mod misc_token;

// Header specific
mod mailbox;
mod address;
mod identification;

// Header blocks
pub mod common_fields;
pub mod trace;

// Global mail
pub mod header;


