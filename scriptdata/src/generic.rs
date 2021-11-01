use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::rc::Rc;

use roxmltree::{Document as RoxDocument, Node as RoxNode};
use xmlwriter::XmlWriter;

use pd2tools_macros::EnumFromData;
use crate::document::{DocumentBuilder, DocumentRef, InteriorTableWriter, TableRef};
use crate::{BorrowedKey, Item, Scalar, ScalarItem, SchemaError, TableId};

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {
    let rn = doc.root_element();
    rn.assert_name("generic_scriptdata")?;

    let builder = DocumentBuilder::default();
    
    let root_data = load_value(&rn)?;
    match root_data {
        LoadScalarResult::Bool(val) => Ok(builder.bool_document(val)),
        LoadScalarResult::Number(val) => Ok(builder.number_document(val)),
        LoadScalarResult::IdString(val) => Ok(builder.idstring_document(val)),
        LoadScalarResult::String(val) => Ok(builder.string_document(val)),
        LoadScalarResult::Vector(val) => Ok(builder.vector_document(val)),
        LoadScalarResult::Quaternion(val) => Ok(builder.quaternion_document(val)),
        LoadScalarResult::Table => load_root_table(&rn, builder),
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
            Err(_) => Err(SchemaError::InvalidFloat)
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
                (_, None) => Ok(LoadScalarResult::Table),
                (_, Some(r)) => Ok(LoadScalarResult::Ref(r))
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

fn load_root_table<'a, 'input>(node: &RoxNode<'a, 'input>, mut doc_builder: DocumentBuilder) -> Result<DocumentRef, SchemaError> {
    let mut ids = HashMap::<&str, TableId>::new();
    let mut found_ids = HashSet::<&str>::new();
    
    let (builder, _) = doc_builder.table_document();

    load_table(node, &mut ids, &mut found_ids, builder)?;

    //drop(builder);
    Ok(doc_builder.finish())
}

fn load_table<'a, 'input, 't>(node: &RoxNode<'a, 'input>, ids: &mut HashMap<&'a str, TableId>, found_ids: &mut HashSet<&'a str>, mut table: InteriorTableWriter<'t>) -> Result<(), SchemaError> {
    let tid = table.table_id();
    let rid = node.attribute("_id");
    rid.map(|rr| {
        found_ids.insert(rr);
        ids.insert(rr, tid)
    });

    let meta = node.attribute("metatable");
    table.set_meta(meta);

    for n in node.children() {
        n.assert_name("entry")?;

        let key = load_key(&n)?;
        let datum = load_value(&n)?;

        let ew = table.key(key)?;

        match datum {
            LoadScalarResult::Bool(val) => ew.bool(val),
            LoadScalarResult::Number(val) => ew.number(val),
            LoadScalarResult::IdString(val) => ew.idstring(val),
            LoadScalarResult::String(val) => ew.string(val),
            LoadScalarResult::Vector(val) => ew.vector(val),
            LoadScalarResult::Quaternion(val) => ew.quaternion(val),
            LoadScalarResult::Table => {
                let id = node.attribute("_id").and_then(|i| ids.get(i));
                let tb = match id {
                    None => ew.new_table(),
                    Some(tid) => ew.resume_table(*tid).unwrap(),
                };
                load_table(&n, ids, found_ids, tb.1)?
            },
            LoadScalarResult::Ref(rid) => {
                match ids.get(rid) {
                    Some(tid) => { ew.resume_table(*tid).unwrap(); },
                    None => {
                        let (tid, mut tb) = ew.new_table();
                        tb.set_meta(n.attribute("meta"));
                        ids.insert(rid, tid);
                    }
                }
            },
        };
    }
    Ok(())
}

#[derive(EnumFromData)]
enum LoadScalarResult<'s> {
    Bool(bool),
    Number(f32),
    IdString(u64),
    String(&'s str),
    Vector(vek::Vec3<f32>),
    Quaternion(vek::Quaternion<f32>),
    Table,
    
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

pub fn dump(doc: DocumentRef, opts: xmlwriter::Options) -> String {
    let mut xw = XmlWriter::new(opts);
    xw.start_element("generic_scriptdata");
    match doc.root() {
        None => xw.write_attribute("type", "nil"),
        Some(itm) => write_item(&mut xw, itm)
    }
    xw.end_element();
    xw.end_document()
}

macro_rules! wa {
    ($xw:expr, $t:literal, $f:literal, $($fa:expr),*) => { {
        $xw.write_attribute("type", $t);
        $xw.write_attribute_fmt("value", format_args!($f, $($fa),*));
    } }
}

fn write_item(xw: &mut XmlWriter, itm: Item<Rc<str>, TableRef>) {
    match itm {
        Item::Scalar(Scalar::Bool(b)) => wa!(xw, "boolean", "{}", b),
        Item::Scalar(Scalar::Number(n)) => wa!(xw, "number", "{}", n),
        Item::Scalar(Scalar::IdString(ids)) => wa!(xw, "idstring", "{:>016x}", ids),
        Item::Scalar(Scalar::String(s)) => wa!(xw, "string", "{}", s),
        Item::Scalar(Scalar::Vector(v)) => wa!(xw, "vector", "{} {} {}", v.x, v.y, v.z),
        Item::Scalar(Scalar::Quaternion(q)) => wa!(xw, "quaternion", "{} {} {} {}", q.x, q.y, q.z, q.w),
        Item::Table(tab) => {
            xw.write_attribute("type", "table");
        },
    }
}