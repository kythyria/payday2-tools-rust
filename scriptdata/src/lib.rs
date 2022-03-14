pub mod document;
pub mod generic;
mod reference_tree;
pub mod custom;
pub mod lua_like;

use std::borrow::Borrow;
use std::rc::Rc;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableId(usize);
pub struct DuplicateKey(OwnedKey);

#[derive(Debug)]
pub struct DanglingTableId(TableId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key<T> {
    Index(usize),
    String(T)
}
type BorrowedKey<'s> = Key<&'s str>;
type OwnedKey = Key<Rc<str>>;

impl<T> From<usize> for Key<T> {
    fn from(src: usize) -> Key<T> {
        Key::Index(src)
    }
}
impl From<&str> for Key<Rc<str>> {
    fn from(src: &str) -> Self {
        Key::String(src.into())
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Scalar<S> {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(S),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
}
impl<S> From<bool> for Scalar<S> { fn from(s: bool) -> Scalar<S> { Scalar::Bool(s) } }
impl<S> From<f32> for Scalar<S> { fn from(s: f32) -> Scalar<S> { Scalar::Number(s) } }
impl<S> From<u64> for Scalar<S> { fn from(s:u64) -> Scalar<S> { Scalar::IdString(s) } }
impl<S> From<vek::Vec3<f32>> for Scalar<S> { fn from(s: vek::Vec3<f32>) -> Scalar<S> { Scalar::Vector(s) } }
impl<S> From<vek::Quaternion<f32>> for Scalar<S> { fn from(s: vek::Quaternion<f32>) -> Scalar<S> { Scalar::Quaternion(s) } }

impl<S> Scalar<S> {
    pub fn map_string<SO>(self, func: impl FnOnce(S) -> SO) -> Scalar<SO> {
        use Scalar::*;
        match self {
            String(s) => String(func(s)),
            Bool(i) => Bool(i),
            Number(i) => Number(i),
            IdString(i) => IdString(i),
            Vector(i) => Vector(i),
            Quaternion(i) => Quaternion(i),
        } 
    }
}
impl<S: Borrow<str>> Scalar<S>{
    pub fn to_borrowed<'s>(&'s self) -> Scalar<&'s str> {
        use Scalar::*;
        match self {
            String(s) => String(s.borrow()),
            Bool(i) => Bool(*i),
            Number(i) => Number(*i),
            IdString(i) => IdString(*i),
            Vector(i) => Vector(*i),
            Quaternion(i) => Quaternion(*i),
        } 
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Item<S: Borrow<str>, T> {
    Scalar(Scalar<S>),
    Table(T)
}
pub type ScalarItem = Item<Rc<str>, TableId>;

impl<S: Borrow<str>, T> Item<S,T> {
    pub fn map_table<TO>(self, func: impl FnOnce(T) -> TO) -> Item<S, TO> {
        match self {
            Item::Scalar(s) => Item::Scalar(s),
            Item::Table(t) => Item::Table(func(t))
        }
    }
}

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Unexpected element {0:?}")]
    WrongElement (&'static str),

    #[error("Missing attribute {0:?}")]
    MissingAttribute(&'static str),

    #[error("Unexpected attribute {0:?}")]
    UnexpectedAttribute(&'static str),

    #[error("Unrecognised value {0:?}")]
    BadValue(Rc<str>),

    #[error("Unrecognised node type {0:?}")]
    BadType(Rc<str>),

    #[error("Malformed boolean")]
    InvalidBool,

    #[error("Malformed number: {0}")]
    InvalidFloat(#[from] std::num::ParseFloatError),

    #[error("Malformed integer: {0}")]
    InvalidInt(#[from] std::num::ParseIntError),

    #[error("Malformed Idstring")]
    InvalidIdString,

    #[error("Malformed Vector3")]
    InvalidVector,

    #[error("Malformed Quaternion")]
    InvalidQuaternion,

    #[error("Table reference has an ID ({0:?})")]
    TableIdAndRef(Rc<str>),

    #[error("Root table is a reference")]
    RootIsReference,

    #[error("Ref {0:?} is dangling")]
    DanglingReference(Rc<str>),

    #[error("Malformed index {0:?}")]
    BadIndex(Rc<str>),

    #[error("No key specified")]
    NoKey,

    #[error("Both string key ({0:?}) and index ({1:?}) supplied")]
    KeyAndIndex(Rc<str>, Rc<str>),

    #[error("Duplicate key {0:?}")]
    DuplicateKey(OwnedKey),

    #[error("Duplicate id {0:?}")]
    DuplicateId(Rc<str>),

    #[error("Reference to {0:?} has children")]
    RefHasChildren(Rc<str>),

    #[error("Syntax error: {0}")]
    SyntaxError(Box<dyn std::error::Error>)
}
impl<T> From<SchemaError> for Result<T, SchemaError> {
    fn from(src: SchemaError) -> Self {
        Err(src)
    }
}
impl From<DuplicateKey> for SchemaError {
    fn from(src: DuplicateKey) -> Self {
        SchemaError::DuplicateKey(src.0)
    }
}

trait RoxmlNodeExt<'a> {
    fn assert_name(&self, name: &'static str) -> Result<(), SchemaError>;
    fn required_attribute(&self, name: &'static str)-> Result<&'a str, SchemaError>;
}
impl<'a, 'input> RoxmlNodeExt<'a> for roxmltree::Node<'a, 'input> {
    fn assert_name(&self, name: &'static str) -> Result<(), SchemaError> {
        if !self.has_tag_name(name) {
            return Err(SchemaError::WrongElement(name))
        }
        else { Ok(()) }
    }
    fn required_attribute(&self, name: &'static str)-> Result<&'a str, SchemaError> {
        match self.attribute(name) {
            Some(s) => Ok(s),
            None => Err(SchemaError::MissingAttribute(name))
        }
    }
}

/// This only exists because the extension trait version won't pass the lifetime through
fn required_attribute<'a, 'input>(inp: &roxmltree::Node<'a, 'input>, name: &'static str) -> Result<&'a str, SchemaError> {
    match inp.attribute(name) {
        Some(s) => Ok(s),
        None => Err(SchemaError::MissingAttribute(name))
    }
}