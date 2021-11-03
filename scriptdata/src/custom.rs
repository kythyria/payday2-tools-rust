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
//! * If the element name is `value_node`, parse the `value` attribute as a
//!   scalar string and add it to the containing table as the next array-like
//!   entry.
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

use roxmltree::Document as RoxDocument;
use crate::document::DocumentRef;
use crate::{RoxmlNodeExt, Scalar, SchemaError};

pub fn load<'a>(doc: &'a RoxDocument<'a>) -> Result<DocumentRef, SchemaError> {

    match doc.root().tag_name().name() {
        "value_node" => {
            match doc.root().required_attribute("value")? {
                "nil" => Ok(crate::document::DocumentBuilder::new().empty_document()),
                s => {
                    let sca = parse_scalar(s);
                    Ok(crate::document::DocumentBuilder::new().scalar_document(sca))
                }
            }
        },
        meta => {
            todo!("Why on earth do you even want this dreadful sin against file formats")
        }
    }
}

fn parse_scalar(input: &str) -> Scalar<&str> {
    todo!("WHYYYY")
}