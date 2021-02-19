use std::collections::{HashMap, HashSet};
use std::cmp::Ord;
use std::rc::Rc;
use std::str;

use fnv::{FnvHashMap, FnvHashSet};

use crate::hashindex::{Hash as IdString};
use crate::util::ordered_float::OrderedFloat;
use crate::util::rc_cell::{RcCell, WeakCell};

pub struct Document {
    root_value: Option<DocValue>,
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

    pub fn root(&self) -> Option<DocValue> {
        self.root_value.clone()
    }

    pub fn set_root(&mut self, t: Option<DocValue>) { self.root_value = t; }

    pub fn table_refcounts(&self) -> FnvHashMap<WeakCell<DocTable>, u32> {
        let mut counter = FnvHashMap::<WeakCell<DocTable>, u32>::default();

        match self.root() {
            Some(r) => count_table_references(&r, &mut counter),
            None => ()
        };

        return counter;
    }

    pub fn tables_used_repeatedly(&self) -> FnvHashSet<WeakCell<DocTable>> {
        let counter = self.table_refcounts();
        let result : FnvHashSet<WeakCell<DocTable>> = counter.iter()
            .filter_map(|(k,v)| if *v > 1 { Some(k.clone()) } else { None })
            .collect();
        return result;
    }
}

fn count_table_references(item: &DocValue, counter: &mut FnvHashMap<WeakCell<DocTable>, u32>) {
    if let DocValue::Table(tab) = item {
        let down = tab.downgrade();
        let entry = counter.entry(down);
        *entry.or_insert(0) += 1;

        for (_, v) in &*tab.borrow() {
            count_table_references(v, counter);
        }
    }
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
pub enum DocValue {
    // no Nil because it can never occur in a table and thus only as the root and we can just make root() return Option<Value> if that matters.
    Bool(bool),
    Number(OrderedFloat),
    IdString(IdString),
    String(Rc<str>),
    Vector(Vector<OrderedFloat>),
    Quaternion(Quaternion<OrderedFloat>),
    Table(RcCell<DocTable>)
}

#[derive(Default)]
pub struct DocTable {
    metatable: Option<Rc<str>>,
    dict_like: HashMap<DocValue, DocValue>,
    keys_in_order_of_add: Vec<DocValue>
}
impl DocTable {
    pub fn new() -> DocTable { DocTable::default() }
    pub fn insert(&mut self, key: DocValue, value: DocValue) {
        self.keys_in_order_of_add.push(key.clone());
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
impl std::fmt::Debug for DocTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        if let Some(mt) = &self.metatable {
            write!(f, "{:?} ", mt)?;
        }
        write!(f, "{:?}", self.dict_like)
    }
}

impl<'a> std::iter::IntoIterator for &'a DocTable {
    type Item=(&'a DocValue, &'a DocValue);
    type IntoIter=TableIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        TableIterator {
            inner: self.keys_in_order_of_add.iter(),
            dict: &self.dict_like
        }
    }
}

pub struct TableIterator<'a> {
    inner: std::slice::Iter<'a, DocValue>,
    dict: &'a HashMap<DocValue, DocValue>
}
impl<'a> Iterator for TableIterator<'a> {
    type Item = (&'a DocValue, &'a DocValue);
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            None => None,
            Some(k) => self.dict.get_key_value(k)
        }
    }
}
