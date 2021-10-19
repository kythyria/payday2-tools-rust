use std::rc::Rc;
use std::collections::{BTreeMap, HashMap, HashSet};

use pd2tools_macros::EnumFromData;

use crate::ScriptdataWriter;

#[derive(Debug, Clone)]
pub struct DocumentRef(Rc<DocumentData>);
#[derive(Debug)]
struct DocumentData{
    root: Option<ScalarItem>,
    tables: Vec<TableData>
}
impl DocumentRef {
    pub fn root(&self) -> Option<Item> {
        self.0.root.as_ref().and_then(|i| i.with_document(self.clone()))
    }
    pub fn table(&self, id: TableId) -> Option<TableRef> {
        if self.0.tables.len() > id.0 {
            Some(TableRef(Rc::clone(&self.0), id))
        }
        else {
            None
        }
    }
}

#[derive(EnumFromData, Debug, Clone, PartialEq)]
pub enum ScalarItem {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(Rc<str>),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table(TableId)
}
impl ScalarItem{
    pub fn with_document(&self, doc: DocumentRef) -> Option<Item> {
        Some(match self {
            ScalarItem::Bool(i) => Item::Bool(*i),
            ScalarItem::Number(i) => Item::Number(*i),
            ScalarItem::IdString(i) => Item::IdString(*i),
            ScalarItem::String(i) => Item::String(Rc::clone(i)),
            ScalarItem::Vector(i) => Item::Vector(*i),
            ScalarItem::Quaternion(i) => Item::Quaternion(*i),
            ScalarItem::Table(i) => {
                if i.0 >= doc.0.tables.len() { return None }
                else { Item::Table(TableRef(doc.0, *i)) }
            }
        })
    }
}
impl From<Item> for ScalarItem {
    fn from(src: Item) -> Self {
        match src {
            Item::Bool(i) => ScalarItem::Bool(i),
            Item::Number(i) => ScalarItem::Number(i),
            Item::IdString(i) => ScalarItem::IdString(i),
            Item::String(i) => ScalarItem::String(Rc::clone(&i)),
            Item::Vector(i) => ScalarItem::Vector(i),
            Item::Quaternion(i) => ScalarItem::Quaternion(i),
            Item::Table(t) => ScalarItem::Table(t.1),
        }
    }
}

#[derive(EnumFromData, Debug, Clone)]
pub enum Item {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(Rc<str>),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table(TableRef)
}

#[derive(EnumFromData, Debug, Clone)]
pub enum Key<'s> {
    Index(usize),
    String(&'s str)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TableId(usize);

#[derive(Debug, Default)]
struct TableData {
    meta: Option<Rc<str>>,
    numeric: BTreeMap<usize, ScalarItem>,
    stringed: HashMap<Rc<str>, ScalarItem>
}

#[derive(Clone, Debug)]
pub struct TableRef(Rc<DocumentData>, TableId);
impl TableRef {
    pub fn id(&self) -> TableId { self.1 }
    pub fn document(&self) -> DocumentRef { DocumentRef(Rc::clone(&self.0)) }
    pub fn meta(&self) -> Option<Rc<str>> { self.0.tables[0].meta.as_ref().map(|i| Rc::clone(i)) }

    fn table(&self) -> &TableData {
        let tid = (self.1).0;
        &self.0.tables[tid]
    }

    pub fn get_item(&self, key: Key) -> Option<ScalarItem> {
        match key {
            Key::Index(idx) => { self.table().numeric.get(&idx).map(Clone::clone) },
            Key::String(s) => { self.table().stringed.get(s).map(Clone::clone) }
        }
    }

    pub fn get<'s, K: Into<Key<'s>>>(&self, key: K) -> Option<Item> {
        let k = key.into();
        let item = self.get_item(k);
        item.and_then(|i| i.with_document(self.document()))
    }
    
    pub fn integer_pairs(&self) -> impl Iterator<Item=(usize, Item)> { 
        IPairs::new(self.clone())
    }

    pub fn string_pairs<'s>(&'s self) -> impl Iterator<Item=(Rc<str>, Item)> + 's {
        let doc = self.document();
        self.table().stringed.iter().map(move |(k,v)| {
            let kc = k.clone();
            let vi = v.with_document(doc.clone());
            (kc, vi.unwrap())
        })
    }
}

struct IPairs {
    table: TableRef,
    current_idx: usize
}
impl IPairs {
    pub fn new(table: TableRef) -> IPairs{
        IPairs {
            table,
            current_idx: 0
        }
    }
}
impl Iterator for IPairs {
    type Item = (usize, Item);

    fn next(&mut self) -> Option<Self::Item> {
        match self.table.get(self.current_idx + 1) {
            None => None,
            Some(i) => {
                self.current_idx += 1;
                Some((self.current_idx, i))
            }
        }
    }
}

pub struct DocumentBuilder {
    state: BuilderState,
    root: Option<ScalarItem>,
    tables: Vec<TableData>,
    current_table: Vec<TableId>,
    string_cache: HashSet<Rc<str>>
}

impl DocumentBuilder {
    fn new() -> DocumentBuilder {
        DocumentBuilder{
            state: BuilderState::Begin,
            root: None,
            tables: Vec::new(),
            current_table: Vec::new(),
            string_cache: HashSet::new()
        }
    }

    fn intern(&mut self, data: &str) -> Rc<str> {
        match self.string_cache.get(data) {
            Some(s) => s.clone(),
            None => {
                let d = Rc::<str>::from(data);
                self.string_cache.insert(d.clone());
                d
            }
        }
    }

    fn add_table(&mut self, meta: Option<&str>) -> TableId {
        let tid = TableId(self.tables.len());
        let meta = meta.map(|i| self.intern(i));
        self.tables.push(TableData {
            meta, ..Default::default()
        });
        self.current_table.push(tid);
        tid
    }

    fn add_table_with<I>(&mut self, meta: Option<&str>, inserter: I) -> Result<TableId, BuilderError>
    where
        I: FnOnce(&mut Self, TableId) -> Option<ScalarItem>
    {
        let new_tid = self.add_table(meta);
        inserter(self, new_tid);
        self.state = BuilderState::NextKey;
        Ok(new_tid)
    }

    fn become_broken<T>(&mut self, e: BuilderError) -> Result<T, BuilderError> {
        self.state = BuilderState::Broken;
        Err(e)
    }
}

impl ScriptdataWriter for DocumentBuilder {
    type Error = BuilderError;
    type Document = DocumentRef;

    fn key<'s, K: Into<Key<'s>>>(&mut self, key: K) -> Result<(), BuilderError> {
        match &self.state {
            BuilderState::Begin => self.become_broken(BuilderError::KeyAtRoot),
            BuilderState::NextKey => {
                match key.into() {
                    Key::Index(idx) => {
                        let curr_tid = *self.current_table.last().unwrap();
                        if self.tables[curr_tid.0].numeric.contains_key(&idx) {
                            self.become_broken(BuilderError::DuplicateKey)
                        }
                        else {
                            self.state = BuilderState::NextIndexedEntry(idx);
                            Ok(())
                        }
                    },
                    Key::String(str) => {
                        let curr_tid = *self.current_table.last().unwrap();
                        let key = self.intern(str);
                        if self.tables[curr_tid.0].stringed.contains_key(str) {
                            self.become_broken(BuilderError::DuplicateKey)
                        }
                        else {
                            self.state = BuilderState::NextStringedEntry(key);
                            Ok(())
                        }
                    }
                }
            },
            BuilderState::NextIndexedEntry(_) => self.become_broken(BuilderError::MultipleKeys),
            BuilderState::NextStringedEntry(_) => self.become_broken(BuilderError::MultipleKeys),
            BuilderState::End => self.become_broken(BuilderError::KeyAtRoot),
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder")
        }
    }

    fn value<I: Into<self::ScalarItem>>(&mut self, value: I) -> Result<(), Self::Error> {
        match &self.state {
            BuilderState::Begin => {
                let v = value.into();
                if let ScalarItem::Table(_) = v {
                    return self.become_broken(BuilderError::DanglingReference);
                }
                self.root = Some(v);
                self.state = BuilderState::End;
                Ok(())
            }
            BuilderState::NextKey => self.become_broken(BuilderError::NoKeySpecified),
            BuilderState::NextIndexedEntry(idx) => {
                let curr_tid = *self.current_table.last().unwrap();
                let idx = *idx;
                self.tables[curr_tid.0].numeric.insert(idx, value.into());
                self.state = BuilderState::NextKey;
                Ok(())
            },
            BuilderState::NextStringedEntry(st) => {
                let curr_tid = *self.current_table.last().unwrap();
                self.tables[curr_tid.0].stringed.insert(st.clone(), value.into());
                self.state = BuilderState::NextKey;
                Ok(())
            },
            BuilderState::End => self.become_broken(BuilderError::MultipleRoots),
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder"),
        }
    }

    fn begin_table(&mut self, meta: Option<&str>) -> Result<TableId, BuilderError> {
        match &self.state {
            BuilderState::Begin => {
                self.add_table_with(meta, |s, id| { s.root = Some(id.into()); None})
            },
            BuilderState::NextKey => self.become_broken(BuilderError::NoKeySpecified),
            BuilderState::NextIndexedEntry(idx) => {
                let curr_tid = *self.current_table.last().unwrap();
                let idx = *idx;
                self.add_table_with(meta, |s, tid| {
                    s.tables[curr_tid.0].numeric.insert(idx, tid.into())
                })
            },
            BuilderState::NextStringedEntry(st) => {
                let curr_tid = *self.current_table.last().unwrap();
                let st = st.clone();
                self.add_table_with(meta, |s, tid| {
                    s.tables[curr_tid.0].stringed.insert(st, tid.into())
                })
            },
            BuilderState::End => self.become_broken(BuilderError::MultipleRoots),
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder")
        }
    }

    fn end_table(&mut self) -> Result<(), Self::Error> {
        match &self.state {
            BuilderState::Begin => self.become_broken(BuilderError::NoOpenTables),
            BuilderState::NextKey => {
                self.current_table.pop();
                if self.current_table.is_empty() {
                    self.state = BuilderState::End;
                }
                Ok(())
            },
            BuilderState::NextIndexedEntry(_) => self.become_broken(BuilderError::MissingValue),
            BuilderState::NextStringedEntry(_) => self.become_broken(BuilderError::MissingValue),
            BuilderState::End => self.become_broken(BuilderError::NoOpenTables),
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder"),
        }
    }

    fn finish(mut self) -> Result<DocumentRef, BuilderError> {
        match &self.state {
            BuilderState::Begin => {
                let dd = Rc::new(DocumentData {
                    root: None,
                    tables: Vec::default()
                });
                Ok(DocumentRef(dd))
            },
            BuilderState::NextKey => (&mut self).become_broken(BuilderError::OpenTables),
            BuilderState::NextIndexedEntry(_) => (&mut self).become_broken(BuilderError::MissingValue),
            BuilderState::NextStringedEntry(_) => (&mut self).become_broken(BuilderError::MissingValue),
            BuilderState::End => {
                let dd = Rc::new(DocumentData {
                    root: self.root,
                    tables: self.tables
                });
                Ok(DocumentRef(dd))
            },
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder"),
        }
    }
}

enum BuilderState {
    Begin,
    NextKey,
    NextIndexedEntry(usize),
    NextStringedEntry(Rc<str>),
    End,
    Broken
}

pub enum BuilderError {
    KeyAtRoot,
    MultipleRoots,
    NoKeySpecified,
    MultipleKeys,
    DuplicateKey,
    DanglingReference,
    NoOpenTables,
    MissingValue,
    OpenTables
}