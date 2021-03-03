use std::{fs::File, iter::FromIterator, path::Path};
use std::io;
use std::os::windows::fs::FileExt;
use std::rc::Rc;
use fnv::FnvHashSet;

use crate::formats::scriptdata::*;
use crate::bundles::database::{Database, ReadItem};
use crate::diesel_hash::{hash_str as dhash};

pub fn do_scan<W: std::io::Write>(db: &Database, output: &mut W) -> io::Result<()> {
    let to_read = db.filter_key_sort_physical(|key| {
        key.extension.hash == dhash("credits")
        || key.extension.hash == dhash("dialog_index")
        || key.extension.hash == dhash("sequence_manager")
        || key.extension.hash == dhash("continent")
        || (key.extension.hash == dhash("continents") && key.path.text.is_some())
        || (key.extension.hash == dhash("world") && key.path.text.is_some())
        || key.extension.hash == dhash("mission")
    });

    let mut found = do_scan_pass(to_read)?;

    let mut ordered: Vec<Rc<str>> = Vec::from_iter(found.drain());
    ordered.sort();
    for s in &ordered {
        writeln!(output, "{}", s)?;
    }
    Ok(())
}

fn do_scan_pass(to_read: Vec<(&Path, Vec<ReadItem>)>) -> io::Result<FnvHashSet<Rc<str>>> {
    let mut found = FnvHashSet::<Rc<str>>::default();

    for (path, items) in to_read {
        let bundle = File::open(path)?;
        for item in items {
            let mut bytes = Vec::<u8>::with_capacity(item.length);
            bundle.seek_read(&mut bytes, item.offset as u64)?;
            let scanned = do_scan_buffer(bytes, item);
            match scanned {
                Err(e) => eprintln!("Failed reading \"{:?}\": {:?}", item.key, e),
                Ok(Some(v)) => found.extend(v),
                _ => ()
            }
        }
    }
    return Ok(found);
}

fn do_scan_buffer(buf: Vec<u8>, item: ReadItem) -> Result<Option<Vec<Rc<str>>>,()>{
    let doc = crate::formats::scriptdata::binary::from_binary(&buf, false);
    let iter = match item.key.extension.text {
        Some("credits") => scan_credits(&doc),
        Some("dialog_index") => scan_dialog_index(&doc),
        Some("sequence_manager") => scan_sequence_manager(&doc),
        Some("continent") => scan_continent(&doc),
        Some("continents") => scan_continents(&doc, Rc::from(item.key.path.text.unwrap())),
        Some("world") => scan_world(&doc, Rc::from(item.key.path.text.unwrap())),
        Some("mission") => scan_mission(&doc),
        _ => return Ok(None)
    };
    let result = iter.collect::<Vec<_>>();
    return Ok(Some(result));
}

macro_rules! scan3 {
    (@a $chain:tt $id:tt $path:tt |> {$($childs:tt)+} $($rest:tt)* ) => {
        scan3!(@a $chain $id ($path.flat_map(|item| {
            let fm = std::iter::once(item);
            scan3!(@a (std::iter::empty()) (fm.clone()) (fm.clone()) |> $($childs)+ )
        })) $($rest)*)  
    };
    (@a $chain:tt $id:tt $path:tt |> $t:ident ($($arg:expr),*) $($rest:tt)*) => {
        scan3!(@a $chain $id (ops2::$t($path, $($arg),*)) $($rest)* )
    };
    (@a $chain:tt $id:tt $path:tt ; $($rest:tt)*) => {
        scan3!(@a ($chain.chain($path)) $id $id |> $($rest)*)
        
    };
    (@a $chain:tt $id:tt $path:tt) => {
        ($chain.chain($path))
    };
    ($($fname:ident ($($argpiece:tt)*) {$($body:tt)+})+) => {
        $(
            fn $fname<'a>(doc: &'a Document, $($argpiece)*) -> Box<dyn Iterator<Item=Rc<str>> + 'a> {
                let res = scan3![@a (std::iter::empty()) doc doc |> $($body)+];
                return Box::new(res);
            }
        )+
    }
}

scan3! {
    scan_credits() {
        root() |> indexed() |> metatable("image") |> { key("src") ; key("SRC") } |> strings() |> map(|i| Rc::from(i.to_ascii_lowercase()))
    }
    
    scan_dialog_index() {
        root() |> indexed() |> metatable("include") |> key("name") |> strings()
        |> map(|i| Rc::from(format!("gamedata/dialogs/{}", i)))
    }
    scan_sequence_manager() {
        root() 
        |> indexed() |> metatable("unit")
        |> indexed() |> metatable("sequence")
        |> indexed() |> metatable("material_config")
        |> key("name") |> strings() |> fmap(unquote_lua)
    }
    scan_environment() {
        root() |> indexed() |> metatable("data") |> indexed() |> metatable("others") |> {
            key("global_world_overlay_texture") ;
            key("global_texture") ;
            key("global_world_overlay_mask_texture") ;
            key("underlay")
        } |> strings()
    }
    
    scan_continent() {
        root() |> key("instances") |> indexed() |> key("folder") |> strings()
        |> fmap(|i| {
            let trimmed = i.strip_suffix("/world").unwrap_or(&i);
            vec![
                Rc::from(format!("{}/mission", trimmed)),
                Rc::from(format!("{}/cover_data", trimmed)),
                i
            ].into_iter()
        })
        ;

        root() |> key("statics") |> indexed() |> key("unit_data") |> {
            key("name") ;
            key("editable_gui") |> key("font")
        } |> strings()
    }

    scan_continents(path: Rc<str>) {
        root() |> indexed() |> key("name") |> strings() |> map(move |s|{
            Rc::from(format!("{0}/{1}/{1}", parentof(&path), s))
        })
    }

    scan_world(path: Rc<str>) {
        root() |> key("environment") |> {
            key("environment_areas") |> indexed() |> key("environment");
            key("environment_values") |> key("environment") ;
            key("effects") |> indexed() |> key("name")
        } |> strings() ;

        root() |> {
            {
                key("brush") ;
                key("sounds") ;
                key("world_camera") ;
                key("ai_nav_graphs")
            } |> key("file") ;
            key("world_data") |> key("continents_file") ;
            literal_str("cover_data")
        } |> strings() |> map(move |i| Rc::from(format!("{}/{}", parentof(&path), i)))
    }

    scan_mission() {
        root() |> entries() |> key("elements") |> indexed() |> {
            key_equal_str("class", "ElementPlayEffect") |> key("values") |> key("effect");
            key_equal_str("class", "ElementSpawnUnit") |> key("values") |> key("unit_name");
            key_equal_str("class", "ElementLoadDelayed") |> key("values") |> key("unit_name");
            key_equal_str("class", "ElementSpawnCivilian") |> key("values") |> key("enemy");
            key_equal_str("class", "ElementSpawnEnemyDummy") |> key("values") |> key("enemy")
        } |> strings()
    }
}

fn parentof(s: &str) -> &str {
    match s.rfind('/') {
        None => "",
        Some(idx) => &s[..idx]
    }
}

fn unquote_lua(input: Rc<str>) -> Option<Rc<str>> {
    let trimmed = input.trim();
    if trimmed.len() == 0 { return None; }

    let first = input.chars().nth(0);
    match first {
        Some('"') => (),
        Some('\'') => (),
        _ => return None
    };

    let body = trimmed[1..].strip_suffix(first.unwrap())?;
    
    // this is dirty, but the only things you can have in a filename that
    // lua requires quoting you just prefix with a \ anyway.
    Some(Rc::from(body.replace('\\', "")))
}

mod ops2 {
    use crate::formats::scriptdata::*;
    use crate::util::rc_cell::*;
    use std::convert::TryInto;
    use std::rc::Rc;

    pub fn root(input: &Document)-> impl Iterator<Item=DocValue> {
        input.root().into_iter()
    }

    pub fn strings<TIter: Iterator<Item=TIn>, TIn: TryInto<Rc<str>>>(input: TIter) -> impl Iterator<Item=Rc<str>> {
        input.flat_map(|i| i.try_into())
    }

    pub fn indexed<TIter, TIn>(input: TIter) -> impl Iterator<Item=DocValue>
    where
        TIter: Iterator<Item=TIn>,
        TIn: TryInto<RcCell<DocTable>>
    {
        input.flat_map(|i| i.try_into()).flat_map(|i| {
            IndexedValues {
                table: i,
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

    pub fn entries<TIter, TIn>(input: TIter) -> impl Iterator<Item=DocValue>
    where
        TIter: Iterator<Item=TIn>,
        TIn: TryInto<RcCell<DocTable>>
    {
        input.flat_map(|i| i.try_into()).flat_map(|i| {
            TableEntriesThroughCell::new(i)
        })
    }

    pub fn key<TIter, TIn>(input: TIter, name: &str) -> impl Iterator<Item=DocValue>
    where
        TIter: Iterator<Item=TIn>,
        TIn: TryInto<RcCell<DocTable>>
    {
        let n = DocValue::String(Rc::from(name));
        input.flat_map(|i| i.try_into()).flat_map(move |rcct|{
            rcct.borrow().get(&n).map(|v|v.clone())
        })
    }

    pub fn metatable<TIter, TIn>(input: TIter, name: &'static str) -> impl Iterator<Item=RcCell<DocTable>>
    where
        TIter: Iterator<Item=TIn>,
        TIn: TryInto<RcCell<DocTable>>
    {
        input.flat_map(|i| i.try_into()).filter(move |rct| {
            let b = rct.borrow();
            b.get_metatable().map(|mt| mt.to_ascii_lowercase() == name).unwrap_or(false)
        })
    }

    pub fn key_equal_str<TIter, TIn>(input: TIter, name: &'static str, value: &'static str) -> impl Iterator<Item=RcCell<DocTable>>
    where
        TIter: Iterator<Item=TIn>,
        TIn: TryInto<RcCell<DocTable>>
    {
        let key = DocValue::String(Rc::from(name));
        input.flat_map(|i| i.try_into()).filter(move |rct| {
            let b = rct.borrow();
            match b.get(&key) {
                Some(DocValue::String(s)) => s.as_ref() == value,
                _ => false
            }
        })
    }

    pub fn map<I: Iterator, B, F>(input: I, f: F) -> std::iter::Map<I, F>
    where
            F: FnMut(I::Item) -> B
    {
        input.map(f)
    }
    
    pub fn literal_str<TR, TIn>(_: TIn, s: &str) -> std::iter::Once<TR>
    where
        TR: From<Rc<str>>,
    {
        let v = Rc::from(s);
        std::iter::once(TR::from(v))
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