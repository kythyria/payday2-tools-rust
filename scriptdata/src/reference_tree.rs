use std::borrow::{Borrow};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use pd2tools_macros::EnumFromData;
use crate::{Key, Item, Scalar, SchemaError, TableId};
use crate::document::{DocumentBuilder, DocumentRef, InteriorTableWriter, TableRef};

#[derive(EnumFromData, Debug, Clone)]
pub enum Value<S> {
    Scalar(Scalar<S>),
    Table(TableHeader<S>),
    Ref(S),
}

#[derive(Debug, Clone, Copy)]
pub struct TableHeader<S> {
    pub id: Option<S>,
    pub meta: Option<S>
}

#[derive(Debug, Clone)]
pub struct Data<S> {
    pub key: Key<S>,
    pub value: Value<S>
}

pub type Tree<'s> = ego_tree::Tree<Data<&'s str>>;
pub type Node<'t, 's> = ego_tree::NodeRef<'t, Data<&'s str>>;
pub type NodeMut<'t, 's> = ego_tree::NodeMut<'t, Data<&'s str>>;

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

            load_table(root, *head, &mut ids, &mut found_ids, builder)?;

            Ok(doc_builder.finish())
        }
    }
}

fn load_table<'s, 't: 's>(node: Node<'t, 's>, table_header: TableHeader<&'s str>, ids: &mut HashMap<&'s str, TableId>, found_ids: &mut HashSet<&'s str>, mut table: InteriorTableWriter<'_>) -> Result<(), SchemaError> {
    if let Some(id) = table_header.id {
        if !found_ids.insert(id) {
            return Err(SchemaError::DuplicateId(Rc::from(id)))
        }
        ids.insert(id, table.table_id());
    }

    table.set_meta(table_header.meta);

    for cn in node.children() {
        let ew = table.key(cn.value().key.clone())?;
        match &cn.value().value {
            Value::Scalar(it) => ew.scalar(*it),
            Value::Table(tab) => {
                let id = tab.id.and_then(|i| ids.get(i));
                let tb = match id {
                    None => ew.new_table(),
                    Some(tid) => ew.resume_table(*tid).unwrap()
                };
                load_table(cn, *tab, ids, found_ids, tb.1)?
            },
            Value::Ref(r) => {
                match ids.get(r) {
                    Some(tid) => { ew.resume_table(*tid).unwrap(); },
                    None => {
                        let (tid, _) = ew.new_table();
                        ids.insert(r.borrow(), tid);
                    }
                }
            },
        }
    }
    Ok(())
}

pub fn from_document(doc: DocumentRef) -> Option<ego_tree::Tree<Data<Rc<str>>>> {
    match doc.root() {
        None => None,
        Some(Item::Scalar(s)) => { 
            let data = Data {
                key: Key::Index(0), value: Value::Scalar(s)
            };
            Some(ego_tree::Tree::new(data))
        },
        Some(Item::Table(tref)) => {
            let mut state = DocToTreeState::default();
            
            let thead = TableHeader {
                id: None,
                meta: doc.table(tref.id()).unwrap().meta().map(|i| i.clone())
            };

            let mut tree = ego_tree::Tree::<Data<Rc<str>>>::new(Data {
                key: Key::Index(0),
                value: Value::Table(thead)
            });

            state.tree_from_tableref(tref, tree.root_mut());
            state.assign_refids(&mut tree);

            Some(tree)
        }
    }
}

#[derive(Default)]
struct DocToTreeState {
    doc_tid_by_tree_nid: HashMap::<ego_tree::NodeId, TableId>,
    tree_nid_by_doc_tid: HashMap::<TableId, ego_tree::NodeId>,

    /// Key: ref node. Value: node it refers to.
    pending_refs: Vec::<(ego_tree::NodeId, ego_tree::NodeId)>
}

impl DocToTreeState {
    fn add_mapping(&mut self, tid: TableId, nid: ego_tree::NodeId) {
        self.doc_tid_by_tree_nid.insert(nid, tid);
        self.tree_nid_by_doc_tid.insert(tid, nid);
    }

    fn tree_from_tableref(&mut self, tref: TableRef, mut node: ego_tree::NodeMut<Data<Rc<str>>>) {
        self.add_mapping(tref.id(), node.id());
        for (k, v) in tref_entries(&tref) {
            match v {
                Item::Scalar(s) => {
                    node.append(Data {
                        key: k,
                        value: Value::Scalar(s)
                    });
                },
                Item::Table(t) => {
                    if let Some(target) = self.tree_nid_by_doc_tid.get(&t.id()) {
                        let rn = node.append(Data {
                            key: k,
                            value: Value::Ref(Rc::from(""))
                        });
                        self.pending_refs.push((rn.id(), *target));
                    }
                    else {
                        let tn = node.append(Data {
                            key: k,
                            value: Value::Table(TableHeader {
                                id: None,
                                meta: t.meta()
                            })
                        });
                        self.tree_from_tableref(t, tn);
                    }
                }
            };
        }
    }

    fn assign_refids(&mut self, tree: &mut ego_tree::Tree<Data<Rc<str>>>) {
        let mut current_id = 0;
        for (r, t) in &self.pending_refs {
            let target_id = ensure_has_id(tree.get_mut(*t).unwrap(), &mut current_id);
            let mut refnode = tree.get_mut(*r).unwrap();
            refnode.value().value = Value::Ref(target_id)
        }
    }
}

fn ensure_has_id(mut node: ego_tree::NodeMut<Data<Rc<str>>>, current_id: &mut usize) -> Rc<str> {
    match &mut node.value().value {
        Value::Table(tab) => {
            tab.id.get_or_insert_with(|| format!("{}", current_id).into()).clone()
        },
        _ => panic!("Tried to reference a non-table item")
    }
}

fn tref_entries<'a>(tref: &'a TableRef) -> impl Iterator<Item=(Key<Rc<str>>, Item<Rc<str>, TableRef>)> + 'a {
    let spairs = tref.string_pairs().map(|(k,v)| (Key::String(k), v));
    let ipairs = tref.integer_pairs().map(|(k,v)| (Key::Index(k), v));

    spairs.chain(ipairs)
}