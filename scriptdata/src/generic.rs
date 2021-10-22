use std::convert::TryFrom;
use std::collections::HashMap;
use std::str::FromStr;
use std::rc::Rc;

use roxmltree::{Document as RoxDocument, Node as RoxNode};

use pd2tools_macros::EnumFromData;
use crate::document::{DocumentBuilder, DocumentRef, Key, ScalarItem, TableId, TableRef, TablesBuilder};
use crate::{ElementWriter, ReopeningWriter, ScriptdataWriter};
use crate::SchemaError;

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {
    let rn = doc.root_element();
    rn.assert_name("generic_scriptdata")?;

    let mut builder = DocumentBuilder::default();
    
    let root_data = load_value(&rn)?;
    match root_data {
        LoadScalarResult::Bool(val) => Ok(builder.scalar_document(val).unwrap()),
        LoadScalarResult::Number(val) => Ok(builder.scalar_document(val).unwrap()),
        LoadScalarResult::IdString(val) => Ok(builder.scalar_document(val).unwrap()),
        LoadScalarResult::String(val) => {
            let val = builder.intern(val);
            Ok(builder.scalar_document(val).unwrap())
        },
        LoadScalarResult::Vector(val) => Ok(builder.scalar_document(val).unwrap()),
        LoadScalarResult::Quaternion(val) => Ok(builder.scalar_document(val).unwrap()),
        LoadScalarResult::Table { id } => load_root_table(&rn, builder),
        LoadScalarResult::Ref(r) => Err(SchemaError::DanglingReference(r.into())),
    }
}

fn load_value<'a, 'input>(node: &RoxNode<'a, 'input>) -> Result<LoadScalarResult<'a>, SchemaError> {
    match (node.required_attribute("type")?, node.attribute("value")) {
        ("boolean", Some("true")) => Ok(true.into()),
        ("boolean", Some("false")) => Ok(false.into()),
        ("boolean", Some(_)) => Err(SchemaError::InvalidBool),

        ("number", Some(ns)) => match f32::from_str(ns) {
            Ok(n) => Ok(n.into()),
            Err(_) => Err(SchemaError::InvalidBool)
        },

        ("idstring", Some(ids)) => match u64::from_str_radix(ids, 16) {
            Ok(val) => Ok(val.swap_bytes().into()),
            Err(_) => Err(SchemaError::InvalidIdString)
        },

        ("string", Some(str)) => Ok(str.into()),

        ("vector", Some(val)) => {
            let v: Vec<_> = val.split(' ').map(f32::from_str).filter_map(Result::ok).collect();
            if v.len() != 3 {
                Err(SchemaError::InvalidVector)
            }
            else {
                Ok(vek::Vec3::new(v[0], v[1], v[2]).into())
            }
        }

        ("quaternion", Some(val)) => {
            let v: Vec<_> = val.split(' ').map(f32::from_str).filter_map(Result::ok).collect();
            if v.len() != 4 {
                Err(SchemaError::InvalidQuaternion)
            }
            else {
                let q = vek::Quaternion::from_xyzw(v[0], v[1], v[2], v[3]);
                Ok(LoadScalarResult::Quaternion(q))
            }
        }

        ("table", Some(_)) => Err(SchemaError::UnexpectedAttribute("value")),
        ("table", None) => {
            match (node.attribute("_id"), node.attribute("_ref")) {
                (Some(id), Some(_)) => Err(SchemaError::TableIdAndRef(Rc::from(id))),
                (id, None) => Ok(LoadScalarResult::Table{id}),
                (_, Some(r)) => Ok(LoadScalarResult::Ref(r))
            }
        },

        (ty, _) => Err(SchemaError::BadType(Rc::from(ty)))
    }
}

fn load_key<'a, 'input>(node: &RoxNode<'a, 'input>) -> Result<Key<'a>, SchemaError> {
    match (node.attribute("index"), node.attribute("key")) {
        (Some(i), Some(k)) => Err(SchemaError::KeyAndIndex(i.into(), k.into())),
        (Some(i), None) => match usize::from_str_radix(i, 10) {
            Ok(i) => Ok(Key::Index(i)),
            Err(_) => Err(SchemaError::BadIndex(i.into())),
        },
        (None, Some(k)) => Ok(Key::String(k)),
        (None, None) => Err(SchemaError::NoKey)
    }
}

fn load_root_table<'a, 'input>(node: &RoxNode<'a, 'input>, builder: DocumentBuilder) -> Result<DocumentRef, SchemaError> {
    
    let meta = node.attribute("meta");
    let (mut builder, root_tid) = builder.table_document(meta);
    
    let root_rid = node.attribute("_id");
    let mut ids = HashMap::<&str, TableId>::new();
    root_rid.and_then(|rr| ids.insert(rr, root_tid));

    for n in node.children() {
        n.assert_name("entry")?;

        let key = load_key(&n)?;
        let datum = load_value(&n)?;

        let res = match datum {
            LoadScalarResult::Bool(val) => builder.scalar_entry(key, val),
            LoadScalarResult::Number(val) => builder.scalar_entry(key, val),
            LoadScalarResult::IdString(val) => builder.scalar_entry(key, val),
            LoadScalarResult::String(val) => {
                let interned = builder.intern(val);
                builder.scalar_entry(key, interned)
            },
            LoadScalarResult::Vector(val) => builder.scalar_entry(key, val),
            LoadScalarResult::Quaternion(val) => builder.scalar_entry(key, val),
            LoadScalarResult::Table { id } => todo!(),
            LoadScalarResult::Ref(rid) => {
                match ids.get(rid) {
                    Some(tid) => builder.reopen_table(key, *tid),
                    None => {
                        let meta = node.attribute("meta");
                        builder.begin_table(key, meta)
                            .and_then(|tid| {ids.insert(rid, tid); Ok(())})
                    },
                }.and_then(|_| builder.end_table())
            },
        };

        match res {
            Ok(_) => (),
            Err(e) => match e {
                crate::document::BuilderError::MultipleRoots => unreachable!(),
                crate::document::BuilderError::DuplicateKey => return Err(SchemaError::DuplicateKey(n.attribute("key").unwrap().into())),
                crate::document::BuilderError::NoOpenTables => unreachable!(),
                crate::document::BuilderError::DanglingReference => return Err(SchemaError::DanglingReference(n.attribute("_ref").unwrap().into())),
            },
        }
    }

    builder.end_table().unwrap();
    Ok(builder.finish().unwrap())
}

#[derive(EnumFromData)]
enum LoadScalarResult<'s> {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(&'s str),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table { id: Option<&'s str> },
    
    #[no_auto_from]
    Ref(&'s str),
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