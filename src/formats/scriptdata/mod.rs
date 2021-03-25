//! Handling for the various scriptdata formats in PD2/Diesel
//!
//! These ostensibly represent lua tables, with a few restrictions,
//! mostly on what keys are allowed by Diesel's serialiser:
//!
//! |type       | binary | custom | generic |
//! |-----------|--------|--------|---------|
//! |bool       | ok     | broken | broken  |
//! |integer    | ok     | ok     | ok      |
//! |float      | ok     | broken | ok      |
//! |idstring   | ok     | broken | broken  |
//! |string     | ok     | ok     | ok      |
//! |vector     | ok     | broken | broken  |
//! |quaternion | ok     | broken | broken  |
//! |table      | ???    | crash  | crash   |
//!
//! Note that `custom_xml` only supports integer keys for the array-like
//! part of a table, as if using `ipairs()` for this.
//!
//! This implementation does NOT reproduce the broken behaviours. In
//! addition, it supports a lua-like format which may be easier to type
//! by hand.

mod document;
mod id_tracker;
pub use document::*;

pub mod binary;
pub mod lua_like;
pub mod generic_xml;
pub mod custom_xml;
