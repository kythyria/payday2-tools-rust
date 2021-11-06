//! `generic_xml` scriptdata support
//! 
//! This is the easier schema:
//! - The root element is `generic_scriptdata`
//! - All other elements are `entry`
//! - `@type` dictates the type of the entry.
//! - If the root item is nil, it has `@type="nil"`
//! - Any entry has one of `@index` (integer key) or `@key` (string key)
//! - On `table`:
//!     - `@metadata` specifies the metatable string
//!     - `@_ref` is used in lieu of children, pointing at the element with matching `@id`
//!     - Table entries are just its children.

use std::str::FromStr;
use std::rc::Rc;

use roxmltree::{Document as RoxDocument, Node as RoxNode};
use xmlwriter::XmlWriter;

use crate::document::DocumentRef;
use crate::reference_tree as rt;
use crate::{Key, OwnedKey, RoxmlNodeExt, Scalar, SchemaError};

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {
    let rn = doc.root_element();
    rn.assert_name("generic_scriptdata")?;
    
    let root_data = load_value(&rn)?;
    let reftree = match root_data {
        rt::Value::Ref(r) => return Err(SchemaError::DanglingReference(r.into())),
        rt::Value::Scalar(_) => {
            rt::Tree::new(rt::Data {
                key: OwnedKey::Index(0),
                value: root_data
            })
        },
        rt::Value::Table(_) => {
            let mut tree = rt::Tree::new(rt::Data {
                key: OwnedKey::Index(0),
                value: root_data
            });
            load_table(&rn, tree.root_mut())?;
            tree
        }
    };
    rt::to_document(reftree)
}

fn load_value<'a, 'input>(node: &RoxNode<'a, 'input>) -> Result<rt::Value, SchemaError> {
    use rt::Value::Scalar as VS;
    match (node.required_attribute("type")?, node.attribute("value")) {
        ("boolean", Some("true")) => Ok(VS(true.into())),
        ("boolean", Some("false")) => Ok(VS(false.into())),
        ("boolean", Some(_)) => Err(SchemaError::InvalidBool),

        ("number", Some(ns)) => match f32::from_str(ns) {
            Ok(n) => Ok(VS(n.into())),
            Err(_) => Err(SchemaError::InvalidFloat)
        },

        ("idstring", Some(ids)) => match u64::from_str_radix(ids, 16) {
            Ok(val) => Ok(VS(val.swap_bytes().into())),
            Err(_) => Err(SchemaError::InvalidIdString)
        },

        ("string", Some(str)) => Ok(VS(Scalar::String(str.into()))),

        ("vector", Some(val)) => {
            let v: Vec<_> = val.split(' ').map(f32::from_str).filter_map(Result::ok).collect();
            if v.len() != 3 {
                Err(SchemaError::InvalidVector)
            }
            else {
                Ok(VS(vek::Vec3::new(v[0], v[1], v[2]).into()))
            }
        }

        ("quaternion", Some(val)) => {
            let v: Vec<_> = val.split(' ').map(f32::from_str).filter_map(Result::ok).collect();
            if v.len() != 4 {
                Err(SchemaError::InvalidQuaternion)
            }
            else {
                let q = vek::Quaternion::from_xyzw(v[0], v[1], v[2], v[3]);
                Ok(VS(q.into()))
            }
        }

        ("table", Some(_)) => Err(SchemaError::UnexpectedAttribute("value")),
        ("table", None) => {
            match (node.attribute("_id"), node.attribute("_ref")) {
                (Some(id), Some(_)) => Err(SchemaError::TableIdAndRef(Rc::from(id))),
                (id, None) => Ok(rt::Value::Table(rt::TableHeader {
                    id: id.map(Rc::from),
                    meta: node.attribute("metatable").map(Rc::from)
                })),
                (_, Some(r)) => Ok(rt::Value::Ref(r.into()))
            }
        },

        (ty, _) => Err(SchemaError::BadType(Rc::from(ty)))
    }
}

fn load_key<'a, 'input>(node: &RoxNode<'a, 'input>) -> Result<OwnedKey, SchemaError> {
    match (node.attribute("index"), node.attribute("key")) {
        (Some(i), Some(k)) => Err(SchemaError::KeyAndIndex(i.into(), k.into())),
        (Some(i), None) => match usize::from_str_radix(i, 10) {
            Ok(i) => Ok(OwnedKey::Index(i)),
            Err(_) => Err(SchemaError::BadIndex(i.into())),
        },
        (None, Some(k)) => Ok(OwnedKey::String(k.into())),
        (None, None) => Err(SchemaError::NoKey)
    }
}

fn load_table<'t, 'a, 'input>(xml: &RoxNode<'a, 'input>, mut reftree: rt::NodeMut) -> Result<(), SchemaError> {
    for n in xml.children() {
        n.assert_name("entry")?;
        let key = load_key(&n)?;
        let datum = load_value(&n)?;
        match datum {
            rt::Value::Scalar(_) => { reftree.append(rt::Data { key, value: datum }); },
            rt::Value::Ref(_) => { reftree.append(rt::Data {key, value: datum}); },
            rt::Value::Table(_) => {
                let child = reftree.append(rt::Data {key, value: datum});
                load_table(&n, child)?
            }
        };
    }
    Ok(())
}

pub fn dump(doc: DocumentRef) -> String {
    match crate::reference_tree::from_document(doc) {
        None => String::from(r#"<generic_scriptdata type="nil"/>"#),
        Some(tree) => {
            let mut xwo = xmlwriter::Options::default();
            xwo.indent = xmlwriter::Indent::Spaces(4);
            let mut xw = XmlWriter::new(xwo);

            xw.start_element("generic_scriptdata");
            dump_entry(&mut xw, tree.root());
            xw.end_element();

            xw.end_document()
        }
    }
}

fn dump_entry<'t>(xw: &mut XmlWriter, node: rt::Node<'t>) {
    match &node.value().value {
        rt::Value::Scalar(s) => dump_scalar(xw, s),
        rt::Value::Table(t) => {
            xw.write_attribute("type", "table");
            if let Some(id) = &t.id {
                xw.write_attribute("_id", &id);
            }
            if let Some(meta) = &t.meta {
                xw.write_attribute("metadata", &meta);
            }
            for entry in node.children() {
                xw.start_element("entry");
                match &node.value().key {
                    Key::Index(idx) => xw.write_attribute_fmt("index", format_args!("{}", idx)),
                    Key::String(str) => xw.write_attribute("key", &str),
                }
                dump_entry(xw, entry);
                xw.end_element();
            }
        },
        rt::Value::Ref(r) => {
            xw.write_attribute("type", "table");
            xw.write_attribute("_ref", &r);
        },
    }
}

fn dump_scalar(xw: &mut XmlWriter, val: &Scalar<Rc<str>>) {
    macro_rules! wa{
        ($ty:literal, $fmt:literal, $($fa:expr),*) => {{
            xw.write_attribute("type", $ty);
            xw.write_attribute_fmt("value", format_args!($fmt, $($fa),*));
        }}
    }
    match val {
        Scalar::Bool(v) => wa!("boolean", "{}", v),
        Scalar::Number(v) => wa!("number", "{}", v),
        Scalar::IdString(v) => wa!("idstring", "{:>016x}", v),
        Scalar::String(v) => wa!("string", "{}", v),
        Scalar::Vector(v) => wa!("vector", "{} {} {}", v.x, v.y, v.z),
        Scalar::Quaternion(v) => wa!("quaternion", "{} {} {} {}", v.x, v.y, v.z, v.w),
    }
}