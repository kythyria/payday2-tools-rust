use std::fmt::Display;
use std::rc::Rc;

use fnv::{FnvHashMap, FnvHashSet};
use xmlwriter::*;

use super::document::{Document, DocTable, DocValue};
use crate::util::rc_cell::*;

pub fn dump(doc: &Document) -> String {
    let mut state = DumperState {
        writer: Writer::new(),
        diamond_subjects: doc.tables_used_repeatedly(),
        seen_ids: FnvHashMap::default(),
        next_id: 0
    };
    
    let root = doc.root();
    match root {
        Some(item) => state.write_item(Name::Index(0), &item),
        None => return "<generic_scriptdata type=\"nil\"/>".to_owned()
    }

    state.writer.end_document()
}

struct DumperState {
    writer: Writer,
    diamond_subjects: FnvHashSet<WeakCell<DocTable>>,
    seen_ids: FnvHashMap<WeakCell<DocTable>, Rc<str>>,
    next_id: u32
}
impl DumperState {
    fn write_item(&mut self, name: Name, item: &DocValue) {
        match item {
            DocValue::Bool(b) => self.writer.scalar(name, Type::Boolean, b),
            DocValue::Number(n) => self.writer.scalar(name, Type::Number, n),
            DocValue::IdString(s) => self.writer.scalar(name, Type::IdString, s),
            DocValue::String(s) => self.writer.scalar(name, Type::String, s),
            DocValue::Vector(v) => self.writer.scalar(name, Type::Vector, format_args!("{} {} {}", v.x, v.y, v.z)),
            DocValue::Quaternion(v) => self.writer.scalar(name, Type::Quaternion, format_args!("{} {} {} {}", v.x, v.y, v.z, v.w)),
            DocValue::Table(tr) => self.write_table(name, tr)
        }
    }

    fn write_table(&mut self, name: Name, table: &RcCell<DocTable>) {
        let downgraded = table.downgrade();
        if let Some(id) = self.seen_ids.get(&downgraded) {
            self.writer.xref(name, id);
            return;
        }
        
        let id = if self.diamond_subjects.contains(&downgraded) {
            let entry = self.seen_ids.entry(downgraded);
            Some(match entry {
                std::collections::hash_map::Entry::Occupied(oe) => oe.get().clone(),
                std::collections::hash_map::Entry::Vacant(ve) => {
                    let s = format!("{}", self.next_id);
                    self.next_id += 1;
                    ve.insert(Rc::from(s)).clone()
                }
            })
        }
        else { None };
        
        let table_ref = table.borrow();
        let tab = &*table_ref;

        self.writer.start_table(name, tab.get_metatable().as_deref(), id.as_deref());

        for (k, v) in tab {
            let name = match k {
                DocValue::Number(n) => {
                    if n.0.trunc() == n.0 && n.0 >= 0.0 {
                        Name::Index(n.0 as usize)
                    }
                    else {
                        panic!("generic_xml only supports nonnegative integers and strings as keys");
                    }
                },
                DocValue::String(s) => {
                    Name::Key(s)
                },
                _ => panic!("generic_xml only supports nonnegative integers and strings as keys")
            };

            self.write_item(name, v);
        }

        self.writer.end_entry();
    }
}

enum Type {
    Table, Boolean, Number, Quaternion, Vector, IdString, String
}
enum Name<'a> { Index(usize), Key(&'a str) }
enum Value<V: Display> { Literal(V), Ref(V), None }

struct Writer {
    w: XmlWriter,
    started: bool
}

impl Writer {
    fn new() -> Writer {
        Writer {
            w: XmlWriter::new(Options::default()),
            started: false
        }
    }

    fn start_entry<V: Display>(&mut self,
        name: Name,
        metatable: Option<&str>,
        ty: Type,
        id: Option<&str>,
        value: Value<V>,
    ) {
        if self.started {
            self.w.start_element("entry");
            match name {
                Name::Index(i) => self.w.write_attribute("index", &i),
                Name::Key(k) => self.w.write_attribute("key", k)
            };
        }
        else {
            self.w.start_element("generic_scriptdata");
            self.started = true;
        }

        self.w.write_attribute("type", match ty {
            Type::Table => "table",
            Type::Boolean => "boolean",
            Type::Number => "number",
            Type::Quaternion => "quaternion",
            Type::Vector => "vector",
            Type::IdString => "idstring",
            Type::String => "string"
        });

        match id {
            Some(id) => self.w.write_attribute("_id", id),
            None => ()
        }
        match metatable {
            Some(mt) => self.w.write_attribute("metatable", mt),
            None => ()
        }
        match value {
            Value::Literal(lit) => self.w.write_attribute("value", &lit),
            Value::Ref(r) => self.w.write_attribute("_ref", &r),
            Value::None => ()
        }
    }
    fn end_entry(&mut self) {
        self.w.end_element();
    }

    fn end_document(self) -> String {
        self.w.end_document()
    }

    fn start_table(&mut self, name: Name, metatable: Option<&str>, id: Option<&str>) {
        self.start_entry(name, metatable, Type::Table, id, Value::<&str>::None)
    }

    fn scalar<V: Display>(&mut self, name: Name, ty: Type, value: V) {
        self.start_entry(name, None, ty, None, Value::Literal(value));
        self.end_entry();
    }

    fn xref<V: Display>(&mut self, name: Name, target: V) {
        self.start_entry(name, None, Type::Table, None, Value::Ref(target));
        self.end_entry();
    }
}