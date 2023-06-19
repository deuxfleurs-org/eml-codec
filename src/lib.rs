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
mod trace;
mod datetime;

// Header blocks
pub mod header;

// root
pub mod email;
