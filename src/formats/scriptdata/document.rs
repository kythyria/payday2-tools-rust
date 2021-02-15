use std::collections::{HashMap, HashSet};
use std::cmp::Ord;
use std::rc::Rc;
use std::str;

use crate::hashindex::{Hash as IdString};
use crate::util::ordered_float::OrderedFloat;
use crate::util::rc_cell::RcCell;

pub struct Document {
    root_value: Option<InternalValue>,
    string_cache: HashSet<Rc<str>>
}
impl Document {
    pub fn new() -> Document {
        Document {
            root_value: None,
            string_cache: HashSet::new()
        }
    }

    pub fn cache_string(&mut self, input: &str) -> Rc<str> {
        if let Some(s) = self.string_cache.get(input) {
            return s.clone();
        } 
        else {
            let rcs: Rc<str> = Rc::from(input);
            self.string_cache.insert(rcs.clone());
            return rcs;
        }
    }

    pub fn gc(&mut self) {
        self.string_cache.retain(|item| Rc::strong_count(item) > 1);
    }

    pub fn root(&self) -> Option<InternalValue> {
        self.root_value.clone()
    }

    pub fn set_root(&mut self, t: Option<InternalValue>) { self.root_value = t; }
}
impl std::default::Default for Document {
    fn default() -> Document {
        Document::new()
    }
}

#[derive(Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Debug, Hash)]
pub struct Vector<T> { pub x: T, pub y: T, pub z: T }

#[derive(Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Debug, Hash)]
pub struct Quaternion<T> { pub x: T, pub y: T, pub z: T, pub w: T }

#[derive(Clone, PartialEq, PartialOrd, Ord, Eq, Debug, Hash)]
pub enum InternalValue {
    // no Nil because it can never occur in a table and thus only as the root and we can just make root() return Option<Value> if that matters.
    Bool(bool),
    Number(OrderedFloat),
    IdString(IdString),
    String(Rc<str>),
    Vector(Vector<OrderedFloat>),
    Quaternion(Quaternion<OrderedFloat>),
    Table(RcCell<InternalTable>)
}

#[derive(Default)]
pub struct InternalTable {
    metatable: Option<Rc<str>>,
    dict_like: HashMap<InternalValue, InternalValue>
}
impl InternalTable {
    pub fn new() -> InternalTable { InternalTable::default() }
    pub fn insert(&mut self, key: InternalValue, value: InternalValue) {
        self.dict_like.insert(key, value);
    }
    pub fn get_metatable(&self) -> Option<Rc<str>> { self.metatable.clone() }
    pub fn set_metatable<T: Into<Option<Rc<str>>>>(&mut self, newtable: T) {
        self.metatable = newtable.into();
    }

    /// Total number of items in the table
    /// 
    /// Lua's # operator is array_len()
    pub fn len(&self) -> usize {
        self.dict_like.len()
    }
}
impl std::fmt::Debug for InternalTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(mt) = &self.metatable {
            write!(f, "{:?} ", mt)?;
        }
        write!(f, "{:?}", self.dict_like)
    }
}

impl<'a> std::iter::IntoIterator for &'a InternalTable {
    type Item=(&'a InternalValue, &'a InternalValue);
    type IntoIter=TableIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        TableIterator {
            inner: self.dict_like.iter()
        }
    }
}

pub struct TableIterator<'a> {
    inner: std::collections::hash_map::Iter<'a, InternalValue, InternalValue>
}
impl<'a> Iterator for TableIterator<'a> {
    type Item = (&'a InternalValue, &'a InternalValue);
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
