use std::fmt;
use std::fmt::Write;

use fnv::{FnvHashMap, FnvHashSet};

use super::document::*;
use crate::util::rc_cell::*;

pub fn dump(doc: &Document) -> String {
    let mut output = String::new();
    write!(output, "return ").unwrap();
    match doc.root() {
        Some(item) => {
            let mut state = DumpState {
                output: &mut output,
                seen_table_ids: FnvHashMap::default(),
                referenced_tables: doc.tables_used_repeatedly(),
                next_id: 1
            };
            dump_item(&item, &mut state, 0).unwrap();
        },
        None => write!(output, "nil").unwrap()
    };
    writeln!(output).unwrap();
    output
}

struct DumpState<'o> {
    output: &'o mut String,
    seen_table_ids: FnvHashMap<WeakCell<DocTable>, String>,
    referenced_tables: FnvHashSet<WeakCell<DocTable>>,
    next_id: u32
}

fn dump_item(item: &DocValue, state: &mut DumpState, indent_level: usize) -> Result<(), fmt::Error> {
    match item {
        DocValue::Bool(b) => {
            match b {
                true => write!(state.output, "true"),
                false => write!(state.output, "false")
            }
        },
        DocValue::IdString(ids) => write!(state.output, "IdString(0x{})", ids),
        DocValue::Number(f) => write!(state.output, "{}", f),
        DocValue::Quaternion(q) => write!(state.output, "Quaternion({}, {}, {}, {})", q.x, q.y, q.z, q.w),
        DocValue::Vector(v) => write!(state.output, "Vector3({}, {}, {})", v.x, v.y, v.z),
        DocValue::String(s) => write!(state.output, "{}", WriteLuaString(s)),
        DocValue::Table(tab) => write_lua_table(tab, state, indent_level)
    }
}

struct WriteLuaString<S: AsRef<str>>(S);
impl<S: AsRef<str>> fmt::Display for WriteLuaString<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;
        for c in self.0.as_ref().chars() {
            match c {
                '\x07' => f.write_str("\\a")?,
                '\x08' => f.write_str("\\b")?,
                '\x0C' => f.write_str("\\f")?,
                '\n' => f.write_str("\\n")?,
                '\r' => f.write_str("\\r")?,
                '\t' => f.write_str("\\t")?,
                '\x0B' => f.write_str("\\v")?,
                '\\' => f.write_str("\\")?,
                '\"' => f.write_str("\\\"")?,
                //'\'' => f.write_str("\\'")?,
                c => f.write_char(c)?
            }
        }
        f.write_char('"')
    }
}

fn write_lua_table(table: &RcCell<DocTable>, state: &mut DumpState, indent_level: usize) -> Result<(), fmt::Error> {
    let downgraded = table.downgrade();
    if let Some(id) = state.seen_table_ids.get(&downgraded) {
        write!(state.output, "Ref(\'{}\')", id)?;
    }
    else {
        if state.referenced_tables.contains(&downgraded) {
            write!(state.output, "RefId(\'{}\', ", state.next_id)?;
        }
        state.seen_table_ids.insert(downgraded.clone(), state.next_id.to_string());
        state.next_id += 1;
        let tref = &*table.borrow();
        if let Some(mt) = tref.get_metatable() {
            write!(state.output, "{} ", mt)?;
        }
        write!(state.output, "{{")?;

        if tref.len() == 0 {
            write!(state.output, " ")?;
        }
        else {
            writeln!(state.output)?;
            for (k, v) in tref {
                write_indent(state.output, indent_level+1)?;
                write_key(k, state, indent_level+1)?;
                write!(state.output, " = ")?;
                dump_item(v, state, indent_level+1)?;
                writeln!(state.output, ",")?;
            }
            write_indent(state.output, indent_level)?;
        }

        write!(state.output, "}}")?;
        if state.referenced_tables.contains(&downgraded) {
            write!(state.output, ")")?;
        }
    }
    Ok(())
}

fn write_indent<O: Write>(output: &mut O, level: usize) -> Result<(), fmt::Error> {
    for _ in 0..level {
        write!(output, "  ")?
    }
    Ok(())
}

fn write_key(item: &DocValue, state: &mut DumpState, indent_level: usize) -> Result<(), fmt::Error> {
    match item {
        DocValue::String(s) => {
            if is_valid_ident(s) {
                write!(state.output, "{}", s)?;
                return Ok(());
            }
        }
        _ => {}
    }
    write!(state.output, "[")?;
    dump_item(item, state, indent_level)?;
    write!(state.output, "]")?;
    Ok(())
}

const LUA_KEYWORDS: &[&str] = &[
    "and",       "break",     "do",        "else",      "elseif",
    "end",       "false",     "for",       "function",  "if",
    "in",        "local",     "nil",       "not",       "or",
    "repeat",    "return",    "then",      "true",      "until",     "while"
];

fn is_valid_ident<S: AsRef<str>>(s: S) -> bool {
    let sr = s.as_ref();
    if sr.len() == 0 { return false; }
    for c in sr.chars() {
        if !(char::is_alphanumeric(c) || c == '_') { return false; }
    }
    if LUA_KEYWORDS.contains(&sr) { return false; }
    if let Some(fc) = sr.chars().next() {
        if !(char::is_alphabetic(fc) || fc == '_') { return false; }
    }
    return true;
}