use std::borrow::Borrow;
use std::rc::Rc;
use crate::document::TableRef;
use crate::TableId;

pub enum Scalar<S: Borrow<str>> {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(S),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
}

pub enum Item<S: Borrow<str>, T> {
    Scalar(Scalar<S>),
    Table(T)
}
pub type ScalarItem = Item<Rc<str>, TableId>;

pub enum RefTreeItem<S: Borrow<str>> {
    Scalar(Scalar<S>),
    Table,
    Ref(S)
}

// Actually contained in the document
pub enum ScalarValue {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(Rc<str>),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table(TableId) // ID of table in the document's array
}

// Shown to callers of the document
pub enum DocValue {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(Rc<str>),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table(TableRef)
}

// Used while reading from the XML form
pub enum LoadValueResult<'s> {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(&'s str),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table,
    Ref(&'s str),
}

pub enum Key<T: Borrow<str>> {
    Index(usize),
    String(T)
}