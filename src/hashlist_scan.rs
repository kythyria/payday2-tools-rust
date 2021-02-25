use crate::formats::scriptdata::*;
use std::rc::Rc;
use crate::util::rc_cell::*;

macro_rules! scan_scriptdata {
    (@a $accum:tt $t:ident ($($arg:expr),*)) => { ops::$t($accum $(,$arg)*) };
    (@a $accum:tt $t:ident ($($arg:expr),*)|>$($rest:tt)+) => {
        scan_scriptdata!(@a (ops::$t($accum $(,$arg)*)) $($rest)+ )
    };
    (@func $fname:ident {$($rest:tt)+}) => { 
        fn $fname<'a>(doc: &'a Document) -> Box<dyn Iterator<Item=Rc<str>> + 'a> {
            Box::new(scan_scriptdata!(@a (doc) $($rest)+))
        }
    };
    ($($fname:ident $body:tt)+) => {
        $(scan_scriptdata!(@func $fname $body);)+
    }
}

scan_scriptdata! {
    scan_credits {
        root_table() |> indexed() |> has_metatable("image") |> key("src") |> strings() 
    }
    scan_dialog_index {
        root_table() |> indexed() |> has_metatable("include") |> key("name") |> strings()
        |> map(|i| Rc::from(format!("gamedata/dialogs/{}", i)))
    }
    scan_sequence_manager {
        root_table() 
        |> indexed() |> has_metatable("unit")
        |> indexed() |> has_metatable("sequence")
        |> indexed() |> has_metatable("material_config")
        |> key("name") |> strings() |> fmap(unquote_lua)
    }
}

/*fn unquote_lua(input: Rc<str>) -> Option<Rc<str>> {
    let trimmed = input.trim();
    if(trimmed.len() == 0) { return None; }

    let first = input.chars().nth(0);
    let last = input.las;
    if first != last { return None; }
    match first {
        Some('"') => (),
        Some('\'') => (),
        _ => return None
    }


}*/


// with_root(doc).ipairs().metatable("image").key("src").is_str()

/*
Operations needed
    select root table of document
    select indexed entries of table
    select dict entries of table 
    select of type (table or string matter mostly)
And some kind of union of these expressions.

Laziest way to union is to not bother, just iterate and take the union.
Selecting root table can be implied
*/

mod ops {
    use crate::formats::scriptdata::*;
    use crate::util::rc_cell::*;
    use std::convert::TryFrom;
    use std::rc::Rc;

    pub fn root_table(input: &Document) -> impl Iterator<Item=RcCell<DocTable>> {
        let i = match input.root() {
            Some(DocValue::Table(r)) => {
                Some(r.clone())
            },
            _ => None
        }.into_iter();
        i
    }

    pub fn of_type<V: TryFrom<DocValue>, I: Iterator<Item=DocValue>>(input: I) -> impl Iterator<Item=V> {
        input.flat_map(|v|{
            V::try_from(v).ok()
        })
    }

    pub fn strings(input: impl Iterator<Item=DocValue>) -> impl Iterator<Item=Rc<str>> {
        of_type::<Rc<str>, _>(input)
    }

    pub fn indexed(input: impl Iterator<Item=RcCell<DocTable>>) -> impl Iterator<Item=DocValue> {
        input.flat_map(|table| {
            IndexedValues {
                table,
                counter: 0
            }
        })
    }

    pub struct IndexedValues {
        table: RcCell<DocTable>,
        counter: usize
    }
    impl Iterator for IndexedValues {
        type Item = DocValue;
        fn next(&mut self) -> Option<Self::Item> {
            self.counter += 1;
            let r = self.table.borrow();
            match r.get(&DocValue::from(self.counter as f32)) {
                None => None,
                Some(item) => Some(item.clone())
            }
        }
    }

    pub fn has_metatable(input: impl Iterator<Item=DocValue>, name: &'static str) -> impl Iterator<Item=RcCell<DocTable>> {
        of_type::<RcCell<DocTable>,_>(input).filter(move |rct| {
            let b = rct.borrow();
            b.get_metatable().map(|mt| mt.as_ref() == name).unwrap_or(false)
        })
    }

    pub fn key(input: impl Iterator<Item=RcCell<DocTable>>, name: &str) -> impl Iterator<Item=DocValue> {
        let n = DocValue::String(Rc::from(name));
        input.flat_map(move |rcct|{
            rcct.borrow().get(&n).map(|v|v.clone())
        })
    }

    pub fn map<I: Iterator, B, F>(input: I, f: F) -> std::iter::Map<I, F>
    where
            F: FnMut(I::Item) -> B
    {
        input.map(f)
    }

    pub fn fmap<I: Iterator, U: IntoIterator, F>(input: I, f: F) -> std::iter::FlatMap<I, U, F>
    where
        I: Iterator,
        U: IntoIterator,
        F: FnMut(I::Item) -> U
    {
        input.flat_map(f)
    }
}