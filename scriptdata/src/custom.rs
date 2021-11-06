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

// See https://github.com/kythyria/payday2-tools-rust/blob/9bed431c83d00884f918e534ba9ed918773a2503/src/formats/scriptdata/custom_xml.rs
// for the old implementation.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::str::FromStr;

use roxmltree::{Document as RoxDocument, Node as RoxNode};
use crate::document::{DocumentRef, DocumentBuilder, InteriorTableWriter};
use crate::reference_tree as rt;
use crate::{Key, RoxmlNodeExt, Scalar, SchemaError};

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {

    match doc.root().tag_name().name() {
        "value_node" => {
            match doc.root().required_attribute("value")? {
                "nil" => Ok(crate::document::DocumentBuilder::new().empty_document()),
                s => {
                    let sca = parse_scalar(s)?;
                    Ok(crate::document::DocumentBuilder::new().scalar_document(sca))
                }
            }
        },
        _ => {
            let mut loader = Loader::default();

            let mut tree = ego_tree::Tree::<rt::Data::<&str>>::new(rt::Data {
                key: Key::Index(0),
                value: rt::Value::Table(rt::TableHeader {
                    id: None,
                    meta: None
                })
            });

           
            loader.load_table_node(doc.root(), tree.root_mut())?;

            todo!();
        }
    }
}

#[derive(Default)]
struct Loader<'s> {
    used_ids: HashSet::<&'s str>,
    pending_ambiguities: HashMap<ego_tree::NodeId, (ego_tree::NodeId, &'s str)>
}
impl<'a> Loader<'a> {
    fn load_table_node<'input>(&mut self, node: RoxNode<'a, 'input>, mut tree: ego_tree::NodeMut<rt::Data<&'a str>>) -> Result<(), SchemaError> {
        let mut seen_scalars = HashSet::<&str>::default();
        let mut refs_to_add = HashMap::<&str, ego_tree::NodeId>::default();

        let mut id = None;
        let mut meta = None;
        let mut refid = None;

        for att in node.attributes() {
            if seen_scalars.contains(att.name()) {
                return Err(SchemaError::DuplicateKey(att.name().into()))
            }
            match att.name() {
                "_id" => {
                    id = Some(att.value());
                    if !self.used_ids.insert(att.value()) {
                        return Err(SchemaError::DuplicateId(att.value().into()))
                    }
                },
                "_meta" => meta = Some(att.value()),
                "_ref" => refid = Some(att.value()),
                k => {
                    if !seen_scalars.insert(k) {
                        return Err(SchemaError::DuplicateKey(k.into()));
                    }
                    tree.append(rt::Data {
                        key: Key::String(k),
                        value: rt::Value::Scalar(parse_scalar(att.value())?)
                    });
                    
                }
            }
        }

        match &mut tree.value().value {
            rt::Value::Table(tab) => {
                tab.id = id.map(|i| Rc::from(i));
                tab.meta = meta.map(|i| Rc::from(i));
            }
            _ => panic!()
        }

        let mut current_index = 1;
        
        for cn in node.children() {
            if cn.has_tag_name("value_node") {
                let valstr = cn.required_attribute("value")?;
                let val = parse_scalar(valstr)?;
                tree.append(rt::Data{
                    key: Key::Index(current_index),
                    value: rt::Value::Scalar(val)
                });
                current_index += 1;
            }
            else if let Some(rs) = cn.attribute("_ref") {
                tree.append(rt::Data {
                    key: Key::Index(current_index),
                    value: rt::Value::Ref(rs.into())
                });
                current_index += 1;
            }
            else if cn.has_tag_name("table") {
                let tn = tree.append(rt::Data {
                    key: Key::Index(current_index),
                    value: rt::Value::Table(rt::TableHeader {
                        id: None,
                        meta: node.attribute("_meta").map(Rc::from)
                    })
                });
                self.load_table_node(cn, tn)?;
                current_index += 1;
            }
            else {
                let tn = tree.append(rt::Data {
                    key: Key::Index(current_index),
                    value: rt::Value::Table(rt::TableHeader {
                        id: None,
                        meta: Some(node.tag_name().name().into())
                    })
                });
                seen_scalars.insert(node.tag_name().name());
                refs_to_add.insert(node.tag_name().name(), tn.id());
                current_index += 1;
            }
        }

        Ok(())
    }
}

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