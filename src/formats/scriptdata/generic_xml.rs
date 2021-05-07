use std::fmt::Display;
use std::rc::Rc;
use std::str::FromStr;

use fnv::{FnvHashMap, FnvHashSet};
use xmlwriter::*;

use super::document::{Document, DocTable, DocValue};
use super::{TextEvent, SchemaError, TextParseError};
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

pub fn load_events<'a>(doc: &'a roxmltree::Document<'a>) -> Vec<Result<TextEvent<'a>, TextParseError>> {
    let mut output = Vec::new();

    let rn = doc.root_element();
    if rn.has_tag_name("generic_scriptdata") {
        collect_events(rn, &mut output);
    }
    else {
        output.push(Err(SchemaError::WrongElement{expected: "generic_scriptdata"}.at(&rn)));
    }

    return output;
}

fn collect_events<'a, 'input>(node: roxmltree::Node<'a, 'input>, output: &mut Vec<Result<TextEvent<'a>, TextParseError>>) {
    match (node.attribute("index"), node.attribute("key")) {
        (Some(_), Some(_)) => output.push(Err(SchemaError::KeyAndIndex.at(&node))),
        (None, Some(key)) => output.push(Ok(TextEvent::Key(key))),
        (Some(idx), None) => {
            if let Ok(idx) = u32::from_str(idx) {
                output.push(Ok(TextEvent::Index(idx)))
            }
            else {
                output.push(Err(SchemaError::BadIndex.at(&node)))
            }
        },
        (None, None) => {
            if !node.is_root() {
                output.push(Err(SchemaError::NoKeyOrIndex.at(&node)))
            }
        }
    }
    match node.attribute("type") {
        None => output.push( Err(SchemaError::MissingType.at(&node)) ),

        Some("table") => collect_events_table(node, output),

        Some(t) => match node.attribute("value") {
            Some(v) => output.push(collect_events_scalar(t, v).map_err(|e| e.at(&node))),
            None => output.push(Err(SchemaError::MissingValue.at(&node)))
        }
    }
}

fn collect_events_scalar<'a>(ty: &'a str, val: &'a str) -> Result<TextEvent<'a>, SchemaError> {
    match ty {
        "bool" => match val {
            "true" => Ok(TextEvent::Bool(true)),
            "false" => Ok(TextEvent::Bool(false)),
            _ => Err(SchemaError::InvalidBool)
        },
        "number" => match f32::from_str(val) {
            Ok(n) => Ok(TextEvent::Number(n)),
            Err(_) => Err(SchemaError::InvalidFloat)
        },
        "idstring" => {
            if val.len() == 16 {
                if let Ok(val) = u64::from_str_radix(val, 16) {
                    return Ok(TextEvent::IdString(val.swap_bytes()))
                }
            }
            Err(SchemaError::InvalidIdString)
        },
        "string" => Ok(TextEvent::String(val)),
        "vector" => {
            let v: Vec<_> = val.split(' ').map(f32::from_str).filter_map(Result::ok).collect();
            if v.len() != 3 {
                return Err(SchemaError::InvalidVector)
            }

            Ok(TextEvent::Vector(v[0], v[1], v[2]))
        },
        "quaternion" => {
            let v: Vec<_> = val.split(' ').map(f32::from_str).filter_map(Result::ok).collect();
            if v.len() != 4 {
                return Err(SchemaError::InvalidVector)
            }

            Ok(TextEvent::Quaternion(v[0], v[1], v[2], v[3]))
        }
        _ => Err(SchemaError::UnknownItemType)
    }
}

fn collect_events_table<'a, 'input>(node: roxmltree::Node<'a, 'input>, output: &mut Vec<Result<TextEvent<'a>, TextParseError>>) {
    if node.has_attribute("value") {
        output.push(Err(SchemaError::TableHasValue.at(&node)));
        return;
    }
    let r#ref = node.attribute("_ref");
    let id = node.attribute("_id");
    let meta = node.attribute("metatable");

    if r#ref.is_some() && id.is_some() {
        output.push(Err(SchemaError::RefAndId.at(&node)));
    }

    if let Some(r#ref) = r#ref {
        if node.first_element_child().is_some() {
            output.push(Err(SchemaError::RefHasChildren.at(&node)));
            return;
        }

        output.push(Ok(TextEvent::Reference(r#ref)));
        return;
    }

    output.push(Ok(TextEvent::StartTable{
        id, meta
    }));

    let mut mcn = node.first_element_child();
    while let Some(cn) = mcn {
        if cn.has_tag_name("entry") {
            collect_events(cn, output);
        }
        else {
            output.push(Err(SchemaError::WrongElement{expected: "entry"}.at(&cn)));
        }
        mcn = cn.next_sibling_element();
    }

    output.push(Ok(TextEvent::EndTable));
}