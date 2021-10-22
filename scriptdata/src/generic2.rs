use std::borrow::Borrow;
use std::rc::Rc;

type Quaternion = vek::Quaternion<f32>;
type Vec3f = vek::Vec3<f32>;

pub struct TableId(u64);
pub struct DuplicateKey;
pub struct DanglingTableId;

pub trait ScriptdataWriter {
    type Error;
    type Output;
    type ScalarWriter: ScalarWriter<Next=Self::Output>;
    type ElementWriter: EntryWriter + RootWriter<Output=Self::Output>;

    fn intern(&mut self, st: &str) -> Rc<str>;
    fn scalar_document(self) -> Self::ScalarWriter;
    fn table_document(self) -> Self::ElementWriter;
}

pub trait ScalarWriter {
    type Next;

    fn bool(self, b: bool) -> Self::Next;
    fn number(self, n: f32) -> Self::Next;
    fn idstring(self, id: u64) -> Self::Next;
    fn string<S: Borrow<str>>(self, s: &S) -> Self::Next;
    fn vector(self, v: Vec3f) -> Self::Next;
    fn quaternion(self, q: Quaternion) -> Self::Next;
}

pub trait TableWriter {
    type Next;

    fn new_table<S: Borrow<str>>(self, meta: Option<&S>) -> (TableId, Self::Next);
    fn resume_table(self, table: TableId) -> Result<(TableId, Self::Next), DanglingTableId>;
}

pub trait EntryWriter {
    type Error;
    type EntryWriter: TableWriter + ScalarWriter;

    fn indexed(&mut self, idx: usize) -> Result<Self::EntryWriter, DuplicateKey>;
    fn string_keyed<S: Borrow<str>>(&mut self, s: &S) -> Result<Self::EntryWriter, DuplicateKey>;
}

pub trait RootWriter {
    type Output;

    fn finish(self) -> Self::Output;
}