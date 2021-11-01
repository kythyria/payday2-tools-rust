pub mod document;
pub mod generic;

use std::borrow::Borrow;
use std::rc::Rc;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableId(usize);
pub struct DuplicateKey(OwnedKey);

#[derive(Debug)]
pub struct DanglingTableId(TableId);

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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

    #[error("Malformed number")]
    InvalidFloat,

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