use std::str::FromStr;
use std::rc::Rc;

use roxmltree::{Document as RoxDocument, Node as RoxNode};

use crate::document::DocumentRef;
use crate::reference_tree as rt;
use crate::{BorrowedKey, Scalar, SchemaError};

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {
    let rn = doc.root_element();
    rn.assert_name("generic_scriptdata")?;
    
    let root_data = load_value(&rn)?;
    let reftree = match root_data {
        rt::Value::Ref(r) => Err(SchemaError::DanglingReference(r.into())),
        rt::Value::Scalar(_) => {
            Ok(rt::Tree::new(rt::Data {
                key: BorrowedKey::Index(0),
                value: root_data
            }))
        },
        rt::Value::Table(_) => {
            let mut tree = rt::Tree::new(rt::Data {
                key: BorrowedKey::Index(0),
                value: root_data
            });
            load_table2(&rn, tree.root_mut())?;
            Ok(tree)
        }
    };
    reftree.and_then(rt::to_document)
}

fn load_value<'a, 'input>(node: &RoxNode<'a, 'input>) -> Result<rt::Value<'a>, SchemaError> {
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

        ("string", Some(str)) => Ok(VS(Scalar::String(str))),

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
                    id,
                    meta: node.attribute("metatable")
                })),
                (_, Some(r)) => Ok(rt::Value::Ref(r))
            }
        },

        (ty, _) => Err(SchemaError::BadType(Rc::from(ty)))
    }
}

fn load_key<'a, 'input>(node: &RoxNode<'a, 'input>) -> Result<BorrowedKey<'a>, SchemaError> {
    match (node.attribute("index"), node.attribute("key")) {
        (Some(i), Some(k)) => Err(SchemaError::KeyAndIndex(i.into(), k.into())),
        (Some(i), None) => match usize::from_str_radix(i, 10) {
            Ok(i) => Ok(BorrowedKey::Index(i)),
            Err(_) => Err(SchemaError::BadIndex(i.into())),
        },
        (None, Some(k)) => Ok(BorrowedKey::String(k)),
        (None, None) => Err(SchemaError::NoKey)
    }
}

fn load_table2<'t, 'a, 'input>(xml: &RoxNode<'a, 'input>, mut reftree: rt::NodeMut<'t, 'a>) -> Result<(), SchemaError> {
    for n in xml.children() {
        n.assert_name("entry")?;
        let key = load_key(&n)?;
        let datum = load_value(&n)?;
        match datum {
            rt::Value::Scalar(_) => { reftree.append(rt::Data { key, value: datum }); },
            rt::Value::Ref(_) => { reftree.append(rt::Data {key, value: datum}); },
            rt::Value::Table(_) => {
                let child = reftree.append(rt::Data {key, value: datum});
                load_table2(&n, child)?
            }
        };
    }
    Ok(())
}

trait RoxmlNodeExt {
    fn assert_name(&self, name: &'static str) -> Result<(), SchemaError>;
    fn required_attribute(&self, name: &'static str)-> Result<&str, SchemaError>;
}
impl<'a, 'input> RoxmlNodeExt for RoxNode<'a, 'input> {
    fn assert_name(&self, name: &'static str) -> Result<(), SchemaError> {
        if !self.has_tag_name(name) {
            return Err(SchemaError::WrongElement(name))
        }
        else { Ok(()) }
    }
    fn required_attribute(&self, name: &'static str)-> Result<&'a str, SchemaError> {
        match self.attribute(name) {
            Some(s) => Ok(s),
            None => Err(SchemaError::MissingAttribute(name))
        }
    }
}

