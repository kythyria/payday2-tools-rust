use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use ego_tree::NodeRef;

use pd2tools_macros::EnumFromData;
use crate::{BorrowedKey, Scalar, SchemaError, TableId};
use crate::document::{DocumentBuilder, DocumentRef, InteriorTableWriter};

#[derive(EnumFromData, Debug, Clone, Copy)]
pub enum Value<'s> {
    Scalar(Scalar<&'s str>),
    Table(TableHeader<'s>),
    Ref(&'s str),
}

#[derive(Debug, Clone, Copy)]
pub struct TableHeader<'s> {
    pub id: Option<&'s str>,
    pub meta: Option<&'s str>
}

#[derive(Debug, Clone, Copy)]
pub struct Data<'s> {
    pub key: BorrowedKey<'s>,
    pub value: Value<'s>
}

pub type Tree<'s> = ego_tree::Tree<Data<'s>>;
pub type Node<'t, 's> = ego_tree::NodeRef<'t, Data<'s>>;
pub type NodeMut<'t, 's> = ego_tree::NodeMut<'t, Data<'s>>;

pub fn to_document(tree: Tree) -> Result<DocumentRef, SchemaError> {
    let root = tree.root();
    match &root.value().value {
        Value::Scalar(item) => Ok(DocumentBuilder::new().scalar_document(item.clone())),
        Value::Ref(_) => panic!("RefTree construction didn't reject a root Ref before it got here."),
        Value::Table(head) => {
            let mut ids = HashMap::<&str, TableId>::new();
            let mut found_ids = HashSet::<&str>::new();
            let mut doc_builder = DocumentBuilder::new();
            let (builder, _) = doc_builder.table_document();

            load_table(root, head, &mut ids, &mut found_ids, builder)?;

            Ok(doc_builder.finish())
        }
    }
}

fn load_table<'s, 't>(node: NodeRef<Data<'s>>, table_header: &TableHeader<'s>, ids: &mut HashMap<&'s str, TableId>, found_ids: &mut HashSet<&'s str>, mut table: InteriorTableWriter<'t>) -> Result<(), SchemaError> {
    if let Some(id) = table_header.id {
        if !found_ids.insert(id) {
            return Err(SchemaError::DuplicateId(Rc::from(id)))
        }
        ids.insert(id, table.table_id());
    }

    table.set_meta(table_header.meta);

    for cn in node.children() {
        let ew = table.key(cn.value().key.clone())?;
        match cn.value().value {
            Value::Scalar(it) => ew.scalar(it),
            Value::Table(tab) => {
                let id = tab.id.and_then(|i| ids.get(i));
                let tb = match id {
                    None => ew.new_table(),
                    Some(tid) => ew.resume_table(*tid).unwrap()
                };
                load_table(cn, &tab, ids, found_ids, tb.1)?
            },
            Value::Ref(r) => {
                match ids.get(r) {
                    Some(tid) => { ew.resume_table(*tid).unwrap(); },
                    None => {
                        let (tid, _) = ew.new_table();
                        ids.insert(r, tid);
                    }
                }
            },
        }
    }
    Ok(())
}