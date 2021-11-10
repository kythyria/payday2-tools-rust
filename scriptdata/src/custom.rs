//! The `custom_xml` format
//! 
//! This format is cursed. It exists basically because someone at Overkill was *so*
//! terrified of curly brackets that they couldn't write Lua tables by hand to then
//! be read in with, say, `loadstring()` and some environment cleverness, much less
//! actual imperative Lua code. But `generic_xml` is too verbose to hand-write, and
//! so we get this.
//! 
//! Scalar values are weakly typed:
//! * Booleans are written as `true` or `false`.
//! * Numbers the obvious way in decimal.
//! * IdStrings are the hash in hex, preceded by `@ID` and followed by `@`
//! * Vectors are the three components separated by spaces
//! * Quaternions are the four components separated by spaces. XYZW, I think.
//! * nil is `nil`.
//! * Otherwise, it's a string.
//! 
//! To parse an element:
//! * If the element name is `value_node`, parse the `value` attribute as a scalar
//!   string and add it to the containing table as the next array-like entry.
//! * If the element name is `table` it is a table.
//! * Otherwise it is a table whose `_meta` entry is the element name, stored
//!   in binary scriptdata using the `metatable` property. Add it to the
//!   containing table as the next array-like entry *and* as a dict-like
//!   entry whose key is the element name if such does not already exist.
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
//! 
//! This thingy parses in two passes. The first pass turns XML into a [`reference_tree`],
//! the second manifests the ambiguity implied by the above.

// See https://github.com/kythyria/payday2-tools-rust/blob/9bed431c83d00884f918e534ba9ed918773a2503/src/formats/scriptdata/custom_xml.rs
// for the old implementation.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::str::FromStr;

use ego_tree::NodeId;
use roxmltree::{Document as RoxDocument, Node as RoxNode};
use crate::document::DocumentRef;
use crate::reference_tree as rt;
use crate::{Key, RoxmlNodeExt, Scalar, SchemaError};

fn parse_scalar(input: &str) -> Result<Scalar<Rc<str>>, SchemaError> {
    if input == "true" { return Ok(Scalar::Bool(true)) }
    if input == "false" { return Ok(Scalar::Bool(false)) }

    if input.starts_with("@ID") && input.ends_with("@") {
        let hex = &input[3..(input.len()-1)];
        if let Ok(val) = u64::from_str_radix(hex, 16) {
            return Ok(Scalar::IdString(val))
        }
    }

    if let Ok(val) = f32::from_str(input) {
        return Ok(Scalar::Number(val))
    }
    if let Ok(parts) = input.splitn(4, ' ').map(f32::from_str).collect::<Result<Vec<_>,_>>() {
        if parts.len() == 3 {
            let v = vek::Vec3::new(parts[0], parts[1], parts[2]);
            return Ok(Scalar::Vector(v))
        }
        if parts.len() == 4 {
            let q = vek::Vec4::new(parts[0], parts[1], parts[2], parts[3]);
            return Ok(Scalar::Quaternion(q.into()))
        }
    }

    Ok(Scalar::String(input.into()))

}

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {
    if doc.root().has_tag_name("value_node") {
        if let Some("nil") = doc.root().attribute("value") {
            return Ok(crate::document::DocumentBuilder::new().empty_document())
        }
    }

    let mut tree = rt::empty_tree();
    let mut fixups = Vec::<(NodeId, NodeId)>::default();

    load_node(doc.root(), &mut tree.root_mut(), Key::Index(0), &mut fixups)?;

    // At this point, everything should be done except for dict-like table-valued
    // entries. So to save on refactoring we just make up some refs at a waste of
    // allocations. Slow, but I really question your need for high performance in
    // any scenario where this gets invoked.

    let mut seen_refs = HashSet::<Rc<str>>::default();
    for n in tree.nodes() {
        match &n.value().value {
            rt::Value::Scalar(_) => {},
            rt::Value::Table(t) => {
                t.id.as_ref().map(|i| seen_refs.insert(i.clone()));
            },
            rt::Value::Ref(rs) => {
                seen_refs.insert(rs.clone());
            },
        }
    }

    let mut id_counter = 0;
    for (src, dest) in fixups {
        let id = if let rt::Data{value: rt::Value::Table(dh), ..} = tree.get_mut(dest).unwrap().value() {
            if dh.id.is_none() {
                loop {
                    let candidate = format!("id_fixup_{}", id_counter);
                    id_counter += 1;
                    if !seen_refs.contains(candidate.as_str()) {
                        dh.id = Some(candidate.into());
                        break;
                    }
                }
            }
            dh.id.as_ref().unwrap().clone()
        }
        else {
            panic!("Fixup didn't point to a table")
        };

        tree.get_mut(src).unwrap().value().value = rt::Value::Ref(id)
    }

    rt::to_document(tree.root().first_child().unwrap())
}

fn load_node<'t>(elem: RoxNode, parent: &mut rt::NodeMut<'t>, key: Key<Rc<str>>, fixups: &mut Vec<(NodeId, NodeId)>) -> Result<NodeId, SchemaError> {
    if elem.tag_name().name() == "value_node" {
        let valstr = elem.required_attribute("value")?;
        let val = parse_scalar(valstr)?;
        let node = parent.append(rt::Data {
            key,
            value: val.into()
        });
        return Ok(node.id());
    }

    if let Some(refid) = elem.attribute("_ref") {
        if elem.has_children() {
            return Err(SchemaError::RefHasChildren(refid.into()))
        }

        let node = parent.append(rt::Data {
            key,
            value: rt::Value::Ref(refid.into())
        });
        return Ok(node.id());
    }

    let id = elem.attribute("_id").map(Rc::<str>::from);
    let meta = match (elem.tag_name().name(), elem.attribute("_meta")) {
        ("table", None) => None,
        (m, None) => Some(Rc::from(m)),
        (_, Some(m)) => Some(Rc::from(m)),
    };

    let mut node = parent.append(rt::Data{
        key,
        value: rt::Value::Table(rt::TableHeader {
            id, meta
        })
    });

    for attr in elem.attributes() {
        match attr.name() {
            "_id" | "_meta" | "_ref" => {},
            name => {
                let val = parse_scalar(attr.value())?;
                node.append(rt::Data {
                    key: Key::String(name.into()),
                    value: val.into()
                });
            }
        }
    }

    let mut keyed_nodes = HashMap::<&str, ego_tree::NodeId>::default();
    let mut curr_index = 1;
    for child in elem.children() {
        let cid = load_node(child, &mut node, Key::Index(curr_index), fixups)?;
        curr_index += 1;
        
        let element_name = child.tag_name().name();
        if !child.has_tag_name("value_node") && !child.has_tag_name("table") {
            if elem.has_attribute(element_name) {
                return Err(SchemaError::DuplicateKey(element_name.into()));
            }
            keyed_nodes.insert(element_name, cid);
        }
    }

    for (key, target) in keyed_nodes {
        let n = node.append(rt::Data {
            key: key.into(),
            value: rt::Value::Ref("".into())
        });
        fixups.push((n.id(), target));
    }

    Ok(node.id())
}