//! HTTP contract for aetherd: versioned routes, typed schemas, and structured
//! error responses.

pub(crate) mod error;
pub(crate) mod health;
pub(crate) mod openapi;
pub(crate) mod system;

pub(crate) use openapi::ApiDoc;
