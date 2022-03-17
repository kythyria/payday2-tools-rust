use std::{rc::Rc, convert::{TryFrom, TryInto}};

use chumsky::prelude::*;
use logos::{Lexer, Logos};

use crate::{Key, Scalar};

#[derive(Logos, Clone, Debug, Eq, Hash, PartialEq)]
enum Token {
    #[token("return")] KwReturn,
    #[token("true")] KwTrue,
    #[token("false")] KwFalse,

    #[regex("[_[:alpha:]][_[:alpha:][:digit:]]*", lex_ident)]
    Ident(Rc<str>),

    #[regex(r#"["']"#, lex_short_string)]
    #[regex(r#"\[=*\["#, lex_long_string)]
    String(Rc<str>),

    #[regex(r"0[xX][0-9A-Fa-f]+(\.[0-9A-Fa-f]*)?([pP]-?[0-9]+)?", parse_hex_int)]
    #[regex(r#"[0-9]+"#, priority=2, callback=parse_dec_int)]
    Integer(usize),

    #[regex(r"[-+]?[0-9]+(\.[0-9]*)?([eE][-+]?[0-9]+)?", parse_dec_num)]
    Number(HashableFloat),

    #[token("(")] LeftParen,
    #[token(")")] RightParen,
    #[token("{")] LeftBrace,
    #[token("}")] RightBrace,
    #[token("[")] LeftBracket,
    #[token("]")] RightBracket,
    #[token(",")] Comma,
    #[token("=")] Equals,
    #[token(";")] Semicolon,
    
    #[regex(r#"--\[=*\["#, lex_multiline_comment)]
    Comment,

    #[regex("--.*[\r\n]", logos::skip)]
    #[regex(r"[ \t\r\n]+", logos::skip)]
    #[regex("do|and|else|break|elseif|function")]
    #[regex("or|end|then|until|repeat|while"   )]
    #[regex("if|for|goto|local|return|not"     )]
    #[regex("in|nil")]
    #[error]
    Error
}

fn lex_ident(lex: &mut Lexer<Token>) -> Rc<str> {
    Rc::from(lex.slice())
}

fn lex_short_string(lex: &mut Lexer<Token>) -> Result<Rc<str>, ()> {
    #[derive(Logos, Debug, PartialEq)]
    enum StringPart {
        #[token("[\"']")] Quote,
        #[regex(r#"\\[abfnrtv\\"']"#)] CEscape,
        #[regex(r#"\\x[0-9A-Fa-f][0-9A-Fa-f]"#)] HexByte,
        #[regex(r#"\\[0-9]([0-9][0-9]?)?"#)] DecByte,
        #[regex(r#"\\u\{[0-9A-Fa-f]+\}"#)] Unicode,
        #[regex(r#"[^"'\\]+"#)] Plain,
        #[error] Error
    }
    let mut buf = Vec::<u8>::new();
    let eos = lex.slice();
    let mut strlex = StringPart::lexer(lex.remainder());
    let success = loop {
        let sp = match strlex.next() {
            Some(sp) => sp,
            None => break Err(())
        };
        match sp {
            StringPart::Quote => {
                if strlex.slice() == eos {
                    break Ok(());
                }
                else {
                    buf.push(strlex.slice().as_bytes()[0])
                }
            },
            StringPart::CEscape => buf.push(match strlex.slice() {
                "\\a" => 0x07,
                "\\b" => 0x08,
                "\\f" => 0x0C,
                "\\n" => 0x0A,
                "\\r" => 0x0D,
                "\\t" => 0x09,
                "\\v" => 0x0B,
                "\\\\" => 0x5C,
                "\\\"" => 0x22,
                "\\\'" => 0x27,
                _ => panic!("Somehow missed a C-like escape!")
            }),
            StringPart::HexByte => {
                let hex = &strlex.slice()[2..];
                let hv = u8::from_str_radix(hex, 16).unwrap();
                buf.push(hv);
            },
            StringPart::DecByte => {
                let dec = &strlex.slice()[1..];
                let dv = u16::from_str_radix(dec, 10).unwrap();
                if dv > 255 { break Err(()); }
                buf.push(dv as u8);
            },
            StringPart::Unicode => {
                let st = &strlex.slice()[3..];
                let st = &st[..(st.len()-1)];
                if st.len() > 6 { break Err(()); }
                let cv = u32::from_str_radix(st, 16)
                    .ok()
                    .and_then(char::from_u32);
                if let Some(c) = cv {
                    let mut b = [0; 4];
                    c.encode_utf8(&mut b);
                    buf.extend(b);
                }
                else {
                    break Err(())
                }
            },
            StringPart::Plain => buf.extend_from_slice(strlex.slice().as_bytes()),
            StringPart::Error => break Err(()),
        }
    };
    lex.bump(strlex.span().len());
    if success.is_err() { return Err(()); }
    
    match String::from_utf8(buf) {
        Ok(st) => Ok(st.into()),
        Err(_) => Err(()),
    }
}

fn lex_long_string(lex: &mut Lexer<Token>) -> Result<Rc<str>, ()> {
    let end = lex.slice().replace("[", "]");
    match lex.remainder().find(&end) {
        Some(idx) => {
            let data = &lex.remainder()[..idx];
            lex.bump(idx + end.len());
            Ok(data.into())
        },
        None => Err(())
    }
}

fn lex_multiline_comment(lex: &mut Lexer<Token>) -> Result<(),()> {
    let end = lex.slice()[2..].replace("[", "]");
    match lex.remainder().find(&end) {
        Some(idx) => {
            lex.bump(idx + end.len());
            Ok(())
        },
        None => Err(())
    }
}

fn parse_hex_int(lex: &mut Lexer<Token>) -> Result<usize, ()> {
    usize::from_str_radix(&lex.slice()[2..], 16).map_err(|_|())
}

fn parse_dec_int(lex: &mut Lexer<Token>) -> Result<usize, ()> {
    usize::from_str_radix(&lex.slice()[2..], 10).map_err(|_|())
}

fn parse_dec_num(lex: &mut Lexer<Token>) -> Result<HashableFloat, ()> {
    <HashableFloat as std::str::FromStr>::from_str(lex.slice()).map_err(|_|())
}

fn just_ident(ident: &'static str) -> impl Parser<Token, Token, Error=Simple<Token>> {
    filter(move |t| if let Token::Ident(st) = t {
        st.as_ref() == ident
    } else {false})
}

fn just_string() -> impl Parser<Token, Rc<str>, Error=Simple<Token>> {
    select!{ Token::String(st) => st }
}

macro_rules! funcall {
    ($name:literal, $fname:ident : $first:expr $(,$restname:ident : $rest:expr)* => $map:expr) => {
        just_ident($name)
        .then(just(Token::LeftParen))
        .then($first)
        $(
            .then(just(Token::Comma))
            .then($rest)
        )*
        .then(just(Token::RightParen))
        .map(|(funcall!(@mapargs[] $fname $(,$restname)*),_)| $map)
    };
    
    (@mapargs[$($acc:tt)*] $fst:tt $($rest:tt)*) => {
        funcall!(@mapargs[$fst $($acc)*] $($rest)*)
    };
    (@mapargs[$($tt:tt)*]) => { funcall!(@mai $($tt)*) };
    (@mai $first:ident) => { ((_,_), $first) };
    (@mai $last:ident, $($rest:ident),+) => { ((funcall!(@mai $($rest),+), _), $last) }
}


fn document() -> impl Parser<Token, BoxedValue> {
    let float = select! {
        Token::Number(f) => f.0,
        Token::Integer(i) => i as f32 // TODO: does Lua truncate the same way?
    }.labelled("float");

    let vector = funcall!("Vector", x: float, y: float,z: float => {
        Scalar::Vector(vek::Vec3::new(x,y,z))
    });
    
    let quaternion = funcall!("Quaternion", x: float, y: float, z: float, w: float => {
        Scalar::Quaternion(vek::Quaternion::from_xyzw(x, y, z, w))
    });

    let idstring = funcall!("Idstring", st: just_string() => {
        Scalar::IdString(diesel_hash::from_str(&st))
    });

    let ref_fn = funcall!("Ref", r: just_string() => {
        BoxedValue::Ref(r)
    });

    let scalar_literal = select! {
        Token::KwTrue => Scalar::Bool(true),
        Token::KwFalse => Scalar::Bool(false),
        Token::String(s) => Scalar::String(s),
        Token::Number(n) => Scalar::Number(n.0),
        Token::Integer(i) => Scalar::Number(i as f32), // TODO: does Lua truncate the same way?
    }.labelled("scalar_literal");

    let scalar = scalar_literal
        .or(vector)
        .or(quaternion)
        .or(idstring)
        .map(BoxedValue::Scalar)
        .labelled("scalar");
    
    fn table(val: impl Parser<Token, BoxedValue, Error=Simple<Token>> + Clone) -> impl Parser<Token, BoxedTable, Error=Simple<Token>> {
        let ident_keyed = select!{ Token::Ident(id) => Some(Key::String(id)) }
            .then_ignore(just(Token::Equals))
            .then(val.clone())
            .labelled("ident_keyed");
        
        let value_keyed = select! {
            Token::String(s) => Some(Key::String(s)),
            Token::Integer(i) => Some(Key::Index(i))
        }.delimited_by(just(Token::LeftBracket), just(Token::RightBracket))
            .then_ignore(just(Token::Equals))
            .then(val.clone())
            .labelled("value_keyed");

        ident_keyed
        .or(value_keyed)
        .or(val.map(|value| (None, value) ))
        .separated_by(just(Token::Comma).or(just(Token::Semicolon)))
        .allow_trailing()
        .delimited_by(just(Token::LeftBrace), just(Token::RightBrace))
        .map(|childs|{
            let mut ci = 0;
            let mut children = Vec::with_capacity(childs.len());
            for (k, value) in childs {
                let key = match k {
                    Some(s) => s,
                    None => { ci += 1; Key::Index(ci) }
                };
                children.push(BoxedNode { key, value })
            }
            BoxedTable{ id: None, meta: None, children }
        })
        .labelled("table")
    }

    fn meta_table(val: impl Parser<Token, BoxedValue, Error=Simple<Token>> + Clone) -> impl Parser<Token, BoxedTable, Error=Simple<Token>> {
        let meta_fn = funcall!("Meta", meta: just_string(), tab: table(val.clone()) => {
            BoxedTable { meta: Some(meta), ..tab }
        }).labelled("meta_fn");

        select!{ Token::Ident(m) => m }.or_not()
            .then(table(val.clone()))
            .map(|(meta, tab)| BoxedTable { meta, ..tab } )
            .or(meta_fn)
            .labelled("meta_table")
    }
    
    let value: chumsky::recursive::Recursive<Token, BoxedValue, Simple<Token>> = recursive(|value| {

        let id_fn = funcall!("Id", id: just_string(), tab: meta_table(value.clone()) => {
            BoxedTable{ id: Some(id), ..tab }
        }).labelled("id_fn");

        let tables = id_fn.or(meta_table(value.clone())).or(table(value)).map(BoxedValue::Table);

        scalar.or(tables).or(ref_fn)
    });

    just(Token::KwReturn)
    .ignore_then(value)
    .then_ignore(end())
}

#[derive(Clone, Debug)]
struct BoxedNode {
    key: Key<Rc<str>>,
    value: BoxedValue
}

#[derive(Clone, Debug)]
enum BoxedValue {
    Scalar(Scalar<Rc<str>>),
    Table(BoxedTable),
    Ref(Rc<str>)
}

#[derive(Clone, Debug)]
struct BoxedTable {
    id: Option<Rc<str>>,
    meta: Option<Rc<str>>,
    children: Vec<BoxedNode>
}

#[derive(Clone, Copy)]
struct HashableFloat(f32);
impl std::ops::Deref for HashableFloat {
    type Target = f32;
    fn deref(&self) -> &Self::Target { &self.0 }
}
impl std::ops::DerefMut for HashableFloat {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
impl std::hash::Hash for HashableFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_ne_bytes().hash(state)
    }
}
impl Eq for HashableFloat { }
impl PartialEq for HashableFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 || (self.0.is_nan() && other.0.is_nan())
    }
}
impl std::fmt::Debug for HashableFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <f32 as std::fmt::Debug>::fmt(&self.0, f)
    }
}
impl From<f32> for HashableFloat {
    fn from(f: f32) -> Self {
        if f.is_nan() { HashableFloat(f32::NAN) } else { HashableFloat(f) }
    }
}
impl std::str::FromStr for HashableFloat {
    type Err = std::num::ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <f32 as std::str::FromStr>::from_str(s).map(|i| i.into())
    }
}