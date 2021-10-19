use std::rc::Rc;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::thread::Builder;

use pd2tools_macros::EnumFromData;

use crate::{ElementWriter, ReopeningWriter, ScriptdataWriter};

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

pub struct DocumentBuilder();
impl ScriptdataWriter for DocumentBuilder {
    type Error = ();
    type Output = DocumentRef;
    type ElementWriter = TablesBuilder;

    fn scalar_document<I: Into<ScalarItem>>(&mut self, value: I) -> Result<Self::Output, Self::Error> {
        let item = value.into();
        if let ScalarItem::Table(_) = item {
            return Err(());
        }
        let dd = DocumentData {
            root: Some(item.into()),
            tables: Vec::new()
        };
        Ok(DocumentRef(Rc::new(dd)))
    }

    fn table_document(&mut self, meta: Option<&str>) -> Self::ElementWriter {
        let mut sc = HashSet::<Rc<str>>::default();
        let meta = meta.map(|st|{
            let st = Rc::<str>::from(st);
            sc.insert(st.clone());
            st
        });
        TablesBuilder {
            state: BuilderState::NextKey,
            root: Some(ScalarItem::Table(TableId(0))),
            tables: vec![
                TableData {
                    meta,
                    ..Default::default()
                }
            ],
            current_table: vec![ TableId(0) ],
            string_cache: sc
        }
    }
}

pub struct TablesBuilder {
    state: BuilderState,
    root: Option<ScalarItem>,
    tables: Vec<TableData>,
    current_table: Vec<TableId>,
    string_cache: HashSet<Rc<str>>
}

impl TablesBuilder {

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

    fn insert<'s, I: FnOnce(&mut Self) -> ScalarItem>(&mut self, key: Key<'s>, make_item: I) -> Option<ScalarItem> {
        let curr_tid = *self.current_table.last().unwrap();
        let item = make_item(self);
        match key.into() {
            Key::Index(idx) => {
                self.tables[curr_tid.0].numeric.insert(idx, item)
            }
            Key::String(st) => {
                let key = self.intern(st);
                self.tables[curr_tid.0].stringed.insert(key, item)
            }
        }
    }

    fn become_broken<T>(&mut self, e: BuilderError) -> Result<T, BuilderError> {
        self.state = BuilderState::Broken;
        Err(e)
    }
}

impl ElementWriter for TablesBuilder {
    type Error = BuilderError;
    type Output = DocumentRef;

    fn scalar_entry<'s, K, I>(&mut self, key: K, value: I) -> Result<(), Self::Error>
    where K: Into<Key<'s>>, I: Into<self::ScalarItem> {
        if self.current_table.is_empty() {
            return self.become_broken(BuilderError::MultipleRoots);
        }
        match &self.state {
            BuilderState::NextKey => {
                let value = value.into();
                if let ScalarItem::Table(tid) = value {
                    if tid.0 >= self.tables.len() {
                        return Err(BuilderError::DanglingReference)
                    }
                }
                match self.insert(key.into(), |_| value) {
                    Some(_) => self.become_broken(BuilderError::DuplicateKey),
                    None => Ok(())
                }
            },
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder")
        }
    }

    fn begin_table<'s, K>(&mut self, key: K, meta: Option<&'s str>) -> Result<TableId, Self::Error>
    where K: Into<Key<'s>> {
        if self.current_table.is_empty() {
            return self.become_broken(BuilderError::MultipleRoots);
        }
        match &self.state {
            BuilderState::NextKey => {
                let new_tid = TableId(self.tables.len());
                let meta = meta.map(|i| self.intern(i));
                
                let res = self.insert(key.into(), |s| {
                    s.tables.push(TableData {
                        meta, ..Default::default()
                    });
                    s.current_table.push(new_tid);
                    new_tid.into()
                });
                match res {
                    Some(_) => self.become_broken(BuilderError::DuplicateKey),
                    None => Ok(new_tid)
                }
            },
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder")
        }
    }

    fn end_table(&mut self) -> Result<(), Self::Error> {
        if self.current_table.is_empty() {
            return self.become_broken(BuilderError::NoOpenTables);
        }
        match &self.state {
            BuilderState::NextKey => {
                self.current_table.pop();
                Ok(())
            },
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder"),
        }
    }

    fn finish(self) -> Result<DocumentRef, BuilderError> {
        match &self.state {
            BuilderState::NextKey => {
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

impl ReopeningWriter for TablesBuilder {
    fn reopen_table<'s, K>(&mut self, key: K, tid: TableId) -> Result<(), Self::Error>
    where K: Into<Key<'s>>
    {
        match &self.state {
            BuilderState::NextKey => {
                if tid.0 >= self.tables.len() {
                    return Err(BuilderError::DanglingReference)
                }
                let res = self.insert(key.into(), |s| {
                    s.current_table.push(tid);
                    tid.into()
                });
                match res {
                    Some(_) => self.become_broken(BuilderError::DuplicateKey),
                    None => Ok(())
                }
            },
            BuilderState::Broken => panic!("Continued to try to use a broken DocumentBuilder"),
        }
    }
}

enum BuilderState {
    NextKey,
    Broken
}

pub enum BuilderError {
    MultipleRoots,
    DuplicateKey,
    NoOpenTables,
    DanglingReference
}