//! LLM wrapper for NER extraction using llama.cpp.
//!
//! This crate provides Named Entity Recognition (NER) for veterinary drug mentions
//! using Llama 3.2 models via llama.cpp bindings.

pub mod prompts;
pub mod extraction;

pub use extraction::*;
pub use prompts::*;
