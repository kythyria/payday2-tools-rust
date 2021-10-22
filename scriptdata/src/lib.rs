pub mod document;
pub mod generic;
pub mod generic2;

use std::rc::Rc;
use thiserror::Error;
use either::*;

pub trait ScriptdataWriter {
    type Error;
    type Output;
    type ElementWriter: ElementWriter<Output=Self::Output>;

    fn intern(&mut self, st: &str) -> Rc<str>;
    fn scalar_document<I: Into<document::ScalarItem>>(self, value: I) -> Result<Self::Output, Self::Error>;
    fn table_document(self, meta: Option<&str>) -> (Self::ElementWriter, document::TableId);
}

pub trait ElementWriter {
    type Error;
    type Output;

    fn intern(&mut self, st: &str) -> Rc<str>;

    fn scalar_entry<'s, K, I>(&mut self, key: K, value: I) -> Result<(), Self::Error>
    where K: Into<document::Key<'s>>, I: Into<document::ScalarItem>;
    
    fn begin_table<'s, K>(&mut self, key: K, meta: Option<&'s str>) -> Result<document::TableId, Self::Error>
    where K: Into<document::Key<'s>>;

    fn end_table(&mut self)-> Result<(), Self::Error>;

    fn finish(self) -> Result<Self::Output, Self::Error>;
}

pub trait ReopeningWriter: ElementWriter {
    fn reopen_table<'s, K>(&mut self, key: K, tid: document::TableId) -> Result<(), Self::Error>
    where K: Into<document::Key<'s>>;
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
    DuplicateKey(Rc<str>),
}
impl<T> From<SchemaError> for Result<T, SchemaError> {
    fn from(src: SchemaError) -> Self {
        Err(src)
    }
}