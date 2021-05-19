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

#[derive(Debug, Copy, Clone)]
pub enum TextEvent<'a> {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(&'a str),
    Vector(f32, f32, f32),
    Quaternion(f32, f32, f32, f32),
    StartTable{
        id: TextId<'a>,
        meta: Option<&'a str>
    },
    EndTable,
    Reference(TextId<'a>),
    Key(&'a str),
    Index(u32)
}

#[derive(Debug, Copy, Clone, Hash)]
pub enum TextId<'a> {
    Str(&'a str),
    Int(usize),
    None
}
impl<'a> From<Option<&'a str>> for TextId<'a> {
    fn from(src: Option<&'a str>) -> Self {
        match src {
            Some(s) => Self::Str(s),
            None => Self::None
        }
    }
}
impl<'a> From<&'a str> for TextId<'a> {
    fn from(s: &'a str) -> Self {
        Self::Str(s)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SchemaError {
    WrongElement {
        expected: &'static str
    },
    MissingType,
    MissingValue,
    InvalidBool,
    InvalidFloat,
    InvalidIdString,
    InvalidVector,
    InvalidQuaternion,
    UnknownItemType,
    BadIndex,
    KeyAndIndex,
    NoKeyOrIndex,
    TableHasValue,
    RefAndId,
    RefHasChildren
}

impl SchemaError {
    fn at(self, node: &roxmltree::Node) -> TextParseError {
        TextParseError::SchemaError {
            pos: node.document().text_pos_at(node.range().start),
            kind: self
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum TextParseError {
    //DomError(roxmltree::Error),
    SchemaError{
        pos: roxmltree::TextPos,
        kind: SchemaError
    }
}

