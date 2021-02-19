//! Write out the custom_xml format
//! 
//! The format is specified oddly.
//! 
//! To parse a scalar string:
//! * Booleans are written as `true` or `false`.
//! * Numbers the obvious way in decimal.
//! * IdStrings are the hash in hex, preceded by `@ID` and followed by `@`
//! * Vectors are the three components separated by spaces
//! * Quaternions are the four components separated by spaces. XYZW, I think.
//! * nil is `nil`.
//! * Otherwise, it's a string.
//! 
//! To parse an element:
//! * If the element name is `value_node`, parse the `value` attribute as a
//!   scalar string and add it to the containing table as the next array-like
//!   entry.
//! * If the element name is `table` it is a table.
//! * Otherwise it is a table whose `_meta` entry is the element name, stored
//!   in binary scriptdata using the `metatable` property. Add it to the
//!   containing table as the next array-like entry *and* as a dict-like
//!   entry whose key is the element name.
//! * Each attribute of an element representing a table is a dict-like entry
//!   whose key is the attribute name and whose value is the result of
//!   parsing the attribute value as a scalar string.
//! * If the element has no children and instead a `_ref` attribute, it is
//!   another reference to the element with a matching `_id` attribute. This
//!   might not match the referent's `_meta` entry
//! 
//! Diesel will crash if asked to write out a table whose keys are not all
//! numbers or strings, and will ignore any numeric keys which are outside
//! the array-like range or aren't an integer. If a table has `_meta` then
//! its name actually overrides the key.

use std::fmt;
use std::rc::Rc;

use fnv::FnvHashSet;
use xmlwriter::XmlWriter;

use crate::util::rc_cell::*;
use super::document::{Document, DocTable, DocValue};
use super::id_tracker::*;

pub fn dump(doc: &Document) -> String {
    let mut state = DumperState {
        writer: XmlWriter::new(xmlwriter::Options::default()),
        id_tracker: IdTracker::new(doc)
    };

    match doc.root() {
        Some(item) => state.write_item_element(item),
        None => return "<value_node value=\"nil\"/>\n".to_owned()
    };

    state.end()
}

struct DumperState {
    writer: XmlWriter,
    id_tracker: IdTracker
}

impl DumperState {
    fn write_item_element(&mut self, val: DocValue) {
        match val {
            DocValue::Table(tab) => self.write_table_element_named(None, tab),
            _ => {
                self.writer.start_element("value_node");
                self.writer.write_attribute("value", &ScalarValueString(val));
                self.writer.end_element();
            }
        }
    }

    fn write_table_element_named(&mut self, name: Option<&str>, table: RcCell<DocTable>) {
        let tr = table.borrow();
        match tr.get_metatable() {
            Some(s) => self.writer.start_element(&s),
            None => self.writer.start_element(name.unwrap_or("table"))
        };

        let idcheck = self.id_tracker.track_table(&table);
        match idcheck {
            RefCheck::Ref(id) => self.writer.write_attribute("_ref", &id),
            RefCheck::Id(id) => {
                self.writer.write_attribute("_id", &id);
                self.write_table_contents(&tr);
            },
            RefCheck::None => {
                self.write_table_contents(&tr);
            }
        }

        self.writer.end_element();
    }

    fn write_table_contents(&mut self, table: &DocTable) {
        // Need to write out all attribute entries, then all array-like, then any string->table entries
        // that weren't already implied by the array-like one.

        // all attribute entries
        for (key, value) in table {
            if let DocValue::String(k) = key {
                match value {
                    DocValue::Table(_) => (),
                    _ => self.writer.write_attribute(k, &ScalarValueString(value.clone()))
                }
            }
        }

        let mut seen_keys = FnvHashSet::<Rc<str>>::default();

        for (_, value) in table.ipairs() {
            self.write_item_element(value.clone());
            if let DocValue::Table(vt) = value {
                match vt.borrow().get_metatable(){
                    Some(mt) => seen_keys.insert(mt.clone()),
                    None => false
                };
            }
        }

        for (key, value) in table {
            if let DocValue::String(k) = key {
                if seen_keys.contains(k) { continue; }
                match value {
                    DocValue::Table(tab) => {
                        self.write_table_element_named(Some(k), tab.clone());
                        seen_keys.insert(tab.borrow().get_metatable().unwrap_or(k.clone()));
                    }
                    _ => ()
                }
            }
        }
    }

    fn end(self) -> String {
        self.writer.end_document()
    }
}

struct ScalarValueString(DocValue);
impl fmt::Display for ScalarValueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            DocValue::Table(_)      => panic!("Tried to convert a table to a string using scalar rules."),
            DocValue::Bool(b)       => write!(f, "{}", b),
            DocValue::IdString(ids) => write!(f, "@ID{}@", ids),
            DocValue::Number(n)     => write!(f, "{}", n.0),
            DocValue::Quaternion(n) => write!(f, "{} {} {} {}", n.x, n.y, n.z, n.w),
            DocValue::Vector(n)     => write!(f, "{} {} {}", n.x, n.y, n.z),
            DocValue::String(s)     => write!(f, "{}", s)
        }
    }
}