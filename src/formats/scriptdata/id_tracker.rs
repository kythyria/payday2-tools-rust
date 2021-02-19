use std::collections::hash_map::Entry;

use fnv::{FnvHashMap, FnvHashSet};

use crate::util::rc_cell::{RcCell, WeakCell};
use super::{Document, DocTable};

pub enum RefCheck {
    Id(usize),
    Ref(usize),
    None
}

pub struct IdTracker {
    seen_table_ids: FnvHashMap<WeakCell<DocTable>, usize>,
    diamond_subjects: FnvHashSet<WeakCell<DocTable>>,
    next_id: usize
}

impl IdTracker {
    pub fn new(doc: &Document) -> IdTracker {
        IdTracker {
            diamond_subjects: doc.tables_used_repeatedly(),
            seen_table_ids: FnvHashMap::default(),
            next_id: 1
        }
    }

    pub fn track_table(&mut self, table: &RcCell<DocTable>) -> RefCheck {
        let downgraded = table.downgrade();
        if self.diamond_subjects.contains(&downgraded) {
            let entry = self.seen_table_ids.entry(downgraded);
            match entry {
                Entry::Occupied(oe) => RefCheck::Ref(*oe.get()),
                Entry::Vacant(ve) => {
                    let id = self.next_id;
                    self.next_id += 1;
                    ve.insert(id);
                    RefCheck::Id(id)
                }
            }
        }
        else { RefCheck::None }
    }
}