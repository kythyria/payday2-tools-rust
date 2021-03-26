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
use std::fmt::Write;
use std::str::FromStr;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use fnv::{FnvHashMap, FnvHashSet};
use roxmltree;
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

#[derive(Debug)]
enum LoadError {
    NoValue,
    SpuriousAttribute,
    SpuriousContent,
    DanglingRef,
    DuplicateId,
    RootIsRef,
    RootIsBroken
}

pub fn load(src: &str) -> anyhow::Result<Document> {
    match roxmltree::Document::parse(src) {
        Err(e) => bail!(e),
        Ok(in_doc) => {
            let mut loader = Loader::new(&in_doc);
            loader.parse_everything();
            loader.finish()
        }
    }
}

struct PendingRef {
    source: RcCell<DocTable>,
    entry: DocValue,
    position: usize
}

enum ParseNode<'a> {
    Resolved(DocValue),
    Ref(&'a str),
    Err
}

struct Loader<'input> {
    source_doc: &'input roxmltree::Document<'input>,
    output_doc: Document,
    pending_refs: FnvHashMap<&'input str, Vec<PendingRef>>,
    refs: FnvHashMap<&'input str, RcCell<DocTable>>,
    errors: Vec<(LoadError, usize)>,
    current_place: Option<(RcCell<DocTable>, DocValue)>
}

impl<'a> Loader<'a> {
    fn new(source_doc: &'a roxmltree::Document<'a>) -> Loader<'a> {
        Loader {
            source_doc,
            output_doc: Document::new(),
            pending_refs: FnvHashMap::default(),
            refs: FnvHashMap::default(),
            errors: Vec::new(),
            current_place: None
        }
    }

    fn add_pending_ref(&mut self, rname: &'a str, source: RcCell<DocTable>, entry: DocValue, position: usize) {
        let list = self.pending_refs.entry(rname).or_default();
        list.push(PendingRef {
            source, entry, position
        })
    }

    fn resolve_ref(&mut self, refname: &'a str, target: RcCell<DocTable>) {
        if let Some(pends) = self.pending_refs.remove(refname) {
            for pr in pends {
                let mut tab = pr.source.borrow_mut();
                tab.insert(pr.entry, DocValue::Table(target.clone()));
            }
        }
    }

    fn parse_everything(&mut self) {
        let root = self.source_doc.root_element();
        match self.parse_element(root) {
            ParseNode::Resolved(dv) => self.output_doc.set_root(Some(dv)),
            ParseNode::Err => self.errors.push((LoadError::RootIsBroken, root.range().start)),
            ParseNode::Ref(_) => self.errors.push((LoadError::RootIsRef, root.range().start))
        }
    }

    fn parse_element(&mut self, node: roxmltree::Node<'a, 'a>) -> ParseNode<'a> {
        match node.tag_name().name() {
            "value_node" => ParseNode::Resolved(self.parse_value_node(node)),
            _ => self.parse_table(node)
        }
    }

    fn parse_value_node(&mut self, node: roxmltree::Node) -> DocValue {
        if node.attributes().len() > 1 {
            self.errors.push((LoadError::SpuriousAttribute, node.range().start));
            return DocValue::Bool(false);
        }
        if node.has_children() {
            self.errors.push((LoadError::SpuriousContent,node.range().start));
            return DocValue::Bool(false);
        }
        if let Some(val) = node.attribute("value") {
            return parse_scalar(&mut self.output_doc, val);
        }
        else {
            self.errors.push((LoadError::NoValue, node.range().start));
            return DocValue::Bool(false);
        }
    }

    fn parse_table(&mut self, node: roxmltree::Node<'a, 'a>) -> ParseNode<'a> {
        if let Some(refname) = node.attribute("_ref") {
            if node.attributes().len() > 1 {
                self.errors.push((LoadError::SpuriousAttribute, node.range().start));
                return ParseNode::Err;
            }
            if node.has_children() {
                self.errors.push((LoadError::SpuriousContent, node.range().start));
                return ParseNode::Err;
            }

            if let Some(target) = self.refs.get(refname) {
                return ParseNode::Resolved(DocValue::Table(target.clone()));
            }
            else {
                return ParseNode::Ref(refname)
            }
        }

        let tabr = RcCell::<DocTable>::default();
        {
            let mut tab = tabr.borrow_mut();

            if node.tag_name().name() != "table" {
                let mt = self.output_doc.cache_string(node.tag_name().name());
                tab.set_metatable(Some(mt));
            }

            for attr in node.attributes() {
                if attr.name() == "_id" {
                    if self.refs.contains_key(attr.value()) {
                        self.errors.push((LoadError::DuplicateId, attr.range().start));
                    }
                    else {
                        self.resolve_ref(attr.value(), tabr.clone());
                    }
                    continue;
                }

                let val = parse_scalar(&mut self.output_doc, attr.value());
                let key = self.output_doc.cache_string(attr.name());
                tab.insert(DocValue::from(key), val);
            }

            let mut idx = 1.0;
            for n in node.children().filter(|n| n.is_element()) {
                let key_n = DocValue::from(idx);
                let key_s = if n.tag_name().name() != "table" {
                    Some(DocValue::from(self.output_doc.cache_string(n.tag_name().name())))
                }
                else {
                    None
                };

                match self.parse_element(n) {
                    ParseNode::Err => (),
                    ParseNode::Resolved(dv) => {
                        tab.insert(DocValue::from(idx), dv.clone());
                        if let Some(k) = key_s {
                            tab.insert(k, dv);
                        }
                    },
                    ParseNode::Ref(r) => {
                        self.add_pending_ref(r, tabr.clone(), key_n, n.range().start);
                        if let Some(k) = key_s {
                            self.add_pending_ref(r, tabr.clone(), k, n.range().start);
                        }
                    }
                }
                idx += 1.0;
            }
        }

        return ParseNode::Resolved(DocValue::Table(tabr));
    }

    fn finish(mut self) -> anyhow::Result<Document> {
        if self.errors.len() == 0 {
            self.output_doc.gc();
            return Ok(self.output_doc);
        }

        let mut errmsg = String::from("Generic_xml document has bad structure:\n");
        for (err, pos) in self.errors {
            match write!(errmsg, "    {:?} at {}", err, self.source_doc.text_pos_at(pos)) {
                Ok(_) => (),
                Err(_) => panic!("Somehow failed to build a list of error messages. SOMEHOW.")
            }
        }
        Err(anyhow!(errmsg))
    }
}

fn parse_scalar(doc: &mut Document, text: &str) -> DocValue {
    if text == "true" {
        return DocValue::Bool(true)
    }

    if text == "false" {
        return DocValue::Bool(false)
    }

    if text.starts_with("@ID") && text.ends_with("@") {
        let hex = &text[3..(text.len()-1)];
        if let Ok(val) = u64::from_str_radix(hex, 16) {
            return DocValue::IdString(crate::hashindex::Hash(val));
        }
    }

    if let Ok(val) = f32::from_str(text) {
        return DocValue::from(val);
    }

    if let Ok(parts) = text.splitn(4, ' ').map(f32::from_str).collect::<Result<Vec<_>,_>>() {
        if parts.len() == 3 {
            return DocValue::from((parts[0], parts[1], parts[2]));
        }
        if parts.len() == 4 {
            return DocValue::from((parts[0], parts[1], parts[2], parts[3]));
        }
    }

    return DocValue::String(doc.cache_string(text));
}