use crate::Error;
use crate::Error::BadLibraryFoldersSchema as BS;
use crate::vdf::Node;

pub struct LibraryFolders {
    pub libraries: Vec<Library>
}
impl LibraryFolders {
    pub fn from_vdf(node: &Node) -> Result<LibraryFolders, Error> {
        let libnodes = node
            .has_name("libraryfolders")
            .and_then(Node::section_data)
            .ok_or(Error::BadLibraryFoldersSchema)?;

        let mut libraries = Vec::with_capacity(libnodes.len());

        for n in libnodes {
            let id = match usize::from_str_radix(&n.name, 10) {
                Ok(i) => i,
                Err(_) => continue
            };
            let mut path = String::new();
            let mut label = String::new();
            let mut app_ids = Vec::<String>::new();

            let fieldnodes = n.section_data().ok_or(BS)?;
            for fnode in fieldnodes {
                match fnode.name.as_str() {
                    "path" => path = fnode.string_data().ok_or(BS)?.to_owned(),
                    "label" => label = fnode.string_data().ok_or(BS)?.to_owned(),
                    "apps" => app_ids = fnode.section_data()
                        .ok_or(BS)?
                        .iter()
                        .map(|i| i.name.clone())
                        .collect(),
                    _ => continue
                };
            }
            libraries.push(Library {
                id, path, label, app_ids
            })
        }
        Ok(LibraryFolders{ libraries })
    }
}

pub struct Library {
    pub id: usize,
    pub path: String,
    pub label: String,
    pub app_ids: Vec<String>
}

