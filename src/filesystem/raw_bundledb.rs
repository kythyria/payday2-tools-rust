use std::sync::Arc;

use dokan::OperationError;

use crate::bundles::database::Database;
use super::{ReadOnlyFs,FsReadHandle};

struct BundleFs<'a> {
    database: Arc<Database<'a>>
}

impl ReadOnlyFs for BundleFs<'_> {
    fn open_readable(&self, path: &str, stream: &str) -> Result<Arc<dyn FsReadHandle>, OperationError> {
        let firstbs = path.find("\\");
        let deslashed_path = match firstbs {
            Some(0) => &path[1..],
            _ => path
        };
        let forwards_path = deslashed_path.replace('\\', "/");

        let (db_path, lang, extn) = split_path_to_key(&forwards_path);
        unimplemented!();
    }
}

fn split_path_to_key(p: &str) -> (&str, &str, &str) {
    unimplemented!();
}