use std::rc::Rc;
use crate::formats::scriptdata::*;

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
            pub fn $fname<'a>(buf: &[u8], $($argpiece)*) -> Result<Box<dyn Iterator<Item=Rc<str>>>, Box<dyn std::error::Error>> {
                let doc_owned = crate::formats::scriptdata::binary::from_binary(&buf, false)?;
                let doc = &doc_owned;
                let res = scan3![@a (std::iter::empty()) doc doc |> $($body)+];
                return Ok(Box::new(res));
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