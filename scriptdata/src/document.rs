use std::borrow::Borrow;
use std::rc::Rc;
use std::collections::{BTreeMap, HashMap, HashSet};

type Quaternion = vek::Quaternion<f32>;
type Vec3f = vek::Vec3<f32>;

use crate::{BorrowedKey, DanglingTableId, DuplicateKey, Item, OwnedKey, Scalar, ScalarItem, TableId};

#[derive(Debug, Clone)]
pub struct DocumentRef(Rc<DocumentData>);
#[derive(Debug)]
struct DocumentData{
    root: Option<ScalarItem>,
    tables: Vec<TableData>
}
impl DocumentRef {
    pub fn root(&self) -> Option<Item<Rc<str>, TableRef>> {
        self.0.root.as_ref().map(|i| self.refify_table(i.clone()))
    }
    pub fn table(&self, id: TableId) -> Option<TableRef> {
        if self.0.tables.len() > id.0 {
            Some(TableRef(Rc::clone(&self.0), id))
        }
        else {
            None
        }
    }

    fn refify_table<S: Borrow<str>>(&self, item: Item<S, TableId>) -> Item<S, TableRef> {
        item.map_table(|t| {
            if t.0 >= self.0.tables.len() {
                panic!("{:?} somehow points outside of the table it's in", t);
            }
            TableRef(self.0.clone(), t)
        })
    }
}

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

    pub fn get_item(&self, key: BorrowedKey) -> Option<ScalarItem> {
        match key {
            BorrowedKey::Index(idx) => { self.table().numeric.get(&idx).map(Clone::clone) },
            BorrowedKey::String(s) => { self.table().stringed.get(s).map(Clone::clone) }
        }
    }

    pub fn get<'s, K: Into<BorrowedKey<'s>>>(&self, key: K) -> Option<Item<Rc<str>, TableRef>> {
        let k = key.into();
        let item = self.get_item(k);
        item.map(|i| i.map_table(|t| {
            TableRef(self.0.clone(), t)
        }))
    }
    
    pub fn integer_pairs(&self) -> impl Iterator<Item=(usize, Item<Rc<str>, TableRef>)> { 
        IPairs::new(self.clone())
    }

    pub fn string_pairs<'s>(&'s self) -> impl Iterator<Item=(Rc<str>, Item<Rc<str>, TableRef>)> + 's {
        let doc = self.document();
        self.table().stringed.iter().map(move |(k,v)| {
            let kc = k.clone();
            let vc = v.clone();
            let vi = vc.map_table(|t| {
                TableRef(self.0.clone(), t)
            });
            (kc, vi)
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
    type Item = (usize, Item<Rc<str>, TableRef>);

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

#[derive(Default)]
pub struct DocumentBuilder {
    string_cache: HashSet<Rc<str>>,
    tables: Vec<TableData>
}
impl DocumentBuilder {
    pub fn new() -> Self { Default::default() }

    pub fn empty_document(self) -> DocumentRef {
        DocumentRef(Rc::from(DocumentData{root: None, tables: Vec::default()}))
    }

    fn doc(self, item: ScalarItem) -> DocumentRef {
        DocumentRef(Rc::from(DocumentData{
            root: Some(item),
            tables: Vec::default()
        }))
    }

    fn intern<S: Borrow<str>>(&mut self, it: &S) -> Rc<str> {
        match self.string_cache.get(it.borrow()) {
            Some(s) => s.clone(),
            None => {
                let owned = Rc::<str>::from(it.borrow());
                self.string_cache.insert(owned.clone());
                owned
            }
        }
    }

    pub fn scalar_document(self, item: Scalar<Rc<str>>) -> DocumentRef {
        self.doc(ScalarItem::Scalar(item))
    }
    pub fn bool_document(self, b: bool) -> DocumentRef { self.scalar_document(b.into()) }
    pub fn number_document(self, n: f32) -> DocumentRef { self.scalar_document(n.into()) }
    pub fn idstring_document(self, id: u64) -> DocumentRef { self.scalar_document(id.into()) }
    pub fn vector_document(self, v: Vec3f) -> DocumentRef { self.scalar_document(v.into()) }
    pub fn quaternion_document(self, q: Quaternion) -> DocumentRef { self.scalar_document(q.into()) }
    pub fn string_document(self, s: &str) -> DocumentRef {
        self.doc(ScalarItem::Scalar(Scalar::String(Rc::from(s))))
    }

    pub fn table_document<'t>(&'t mut self) -> (InteriorTableWriter<'t>, TableId) {
        self.tables.push(TableData {
            meta: None,
            numeric: BTreeMap::default(),
            stringed: HashMap::default()
        });
        let tw = InteriorTableWriter {
            root: self,
            table: 0
        };
        (tw, TableId(0))
    }

    pub fn finish(self) -> DocumentRef {
        DocumentRef(Rc::from(DocumentData{
            root: Some(ScalarItem::Table(TableId(0))),
            tables: self.tables
        }))
    }
}

pub struct InteriorTableWriter<'t> {
    root: &'t mut DocumentBuilder,
    table: usize,
}
impl<'t> InteriorTableWriter<'t> {
    pub fn table_id(&self) -> TableId { TableId(self.table) }
    pub fn set_meta(&mut self, meta: Option<&str>) {
        let meta = meta.map(|s| self.root.intern(&s));
        self.root.tables[self.table].meta = meta
    }

    pub fn indexed(&mut self, idx: usize) -> Result<EntryWriter<'_>, DuplicateKey> {
        if self.root.tables[self.table].numeric.contains_key(&idx) {
            return Err(DuplicateKey(OwnedKey::Index(idx)))
        }

        Ok(EntryWriter {
            root: self.root,
            table: self.table,
            key: OwnedKey::Index(idx)
        })
    }

    pub fn string_keyed<S: Borrow<str>>(&mut self, s: &S) -> Result<EntryWriter<'_>, DuplicateKey> {
        let key = self.root.intern(s);
        if self.root.tables[self.table].stringed.contains_key(&key) {
            let k = OwnedKey::String(Rc::from(s.borrow()));
            return Err(DuplicateKey(k))
        }

        Ok(EntryWriter {
            root: self.root,
            table: 0,
            key: OwnedKey::String(key)
        })
    }

    pub fn key(&mut self, key: BorrowedKey) -> Result<EntryWriter<'_>, DuplicateKey> {
        match key {
            BorrowedKey::Index(idx) => self.indexed(idx),
            BorrowedKey::String(st) => self.string_keyed(&st),
        }
    }
}

pub struct EntryWriter<'t> {
    root: &'t mut DocumentBuilder,
    table: usize,
    key: OwnedKey,
}
impl<'t> EntryWriter<'t> {
    fn insert_scalar(&mut self, item: ScalarItem) {
        match self.key.clone() {
            OwnedKey::Index(idx) => self.root.tables[self.table].numeric.insert(idx, item),
            OwnedKey::String(st) => self.root.tables[self.table].stringed.insert(st, item)
        };
    }

    pub fn scalar(mut self, it: Scalar<Rc<str>>) { self.insert_scalar(ScalarItem::Scalar(it))}
    pub fn bool(mut self, it: bool) { self.scalar(it.into()); }
    pub fn number(mut self, it: f32) { self.scalar(it.into()); }
    pub fn idstring(mut self, it: u64) { self.scalar(it.into()); }
    pub fn vector(mut self, it: Vec3f) { self.scalar(it.into()); }
    pub fn quaternion(mut self, it: Quaternion) { self.scalar(it.into()); }
    pub fn string(mut self, it: &str) {
        let s = self.root.intern(&it);
        self.insert_scalar(ScalarItem::Scalar(Scalar::String(s)));
    }

    pub fn new_table(mut self) -> (TableId, InteriorTableWriter<'t>) {
        let tid = self.root.tables.len();
        self.root.tables.push(TableData {
            meta: None,
            numeric: BTreeMap::new(), 
            stringed: HashMap::new()
        });
        self.insert_scalar(ScalarItem::Table(TableId(tid)));
        let ier = InteriorTableWriter {
            root: self.root,
            table: tid
        };
        (TableId(tid), ier)
    }

    pub fn resume_table(self, table: TableId) -> Result<(TableId, InteriorTableWriter<'t>), DanglingTableId> {
        if table.0 >= self.root.tables.len() { Err(DanglingTableId(table)) }
        else {
            Ok((table, InteriorTableWriter {
                root: self.root,
                table: table.0
            }))
        }
    }
}