use std::rc::Rc;

use pest::iterators::Pair;
use pest::{Parser, iterators::Pairs, error::Error as PestError};
use pest_derive::Parser;
use thiserror::Error as ThisError;

use crate::document::DocumentRef;
use crate::{Key, Scalar, reference_tree as rt};

#[derive(ThisError, Debug)]
pub enum LuaLikeError {
    #[error("Error parsing Lua-like syntax: {0}")]
    ParseError(#[from] PestError<Rule>),

    #[error("Malformed number: {0}")]
    BadNumber(#[from] std::num::ParseFloatError),

    #[error("Bad string escape")]
    BadEscape,

    #[error("String not UTF-8")]
    BadStringEncoding
}


#[derive(Parser)]
#[grammar = "lua_like.pest"]
struct LualikeParser();

pub fn get_parse(input: &str) -> Result<Pairs<Rule>, PestError<Rule>> {
    LualikeParser::parse(Rule::document, input)
}

pub fn load(input: &str) -> Result<DocumentRef, LuaLikeError> {
    let mut tree = get_parse(input)?;
    
    let p_doc = tree.next().unwrap();
    let v_doc = p_doc.into_inner();

    todo!();
}

fn parse_value_data(pair: Pair<Rule>, key: Key<Rc<str>>, parent: &mut rt::NodeMut) -> Result<(), LuaLikeError> {
    match pair.as_rule() {
        Rule::number => {
            let num: f32 = pair.as_str().parse()?;
            parent.append(rt::Data {
                key,
                value: rt::Value::Scalar(num.into())
            });
        },
        Rule::bool => {
            let b = match pair.as_str() {
                "true" => true,
                "false" => false,
                _ => panic!("Unaccounted for boolean literal")
            };
            parent.append(rt::Data {
                key,
                value: rt::Value::Scalar(b.into())
            });
        }
        Rule::long_string => {
            let st = Rc::from(pair.as_str());
            parent.append(rt::Data {
                key,
                value: Scalar::String(st).into()
            });
        },
        Rule::short_string => {
            let st = parse_short_string(pair)?;
            parent.append(rt::Data {
                key,
                value: Scalar::String(st).into()
            });
        },
        Rule::table => todo!(),
        Rule::call => todo!(),
        _ => panic!("Unexpected variation between the grammmar of `value` and its handling")
    }
    Ok(())
}

fn parse_table(pair: Pair<Rule>, key: Key<Rc<str>>, parent: &mut rt::NodeMut) -> Result<(), LuaLikeError> {
    let mut node = parent.append(rt::Data {
        key,
        value: rt::Value::Table(rt::TableHeader {
            id: None,
            meta: None
        })
    });

    fill_table(pair, &mut node)
}

fn fill_table(pair: Pair<Rule>, table_node: &mut rt::NodeMut) -> Result<(), LuaLikeError> {
    for p in pair.into_inner() {
        match p.as_rule() {
            Rule::ident_keyed => {
                let mut k = p.into_inner();
                let id = k.next().unwrap();
                let val = k.next().unwrap();
                let val_data = val.into_inner().next().unwrap();
                let key = Key::String(Rc::from(id.as_str()));
                parse_value_data(val_data, key, table_node)?;
            },
            Rule::value_keyed => {
                todo!()
            },
            Rule::value => todo!(),
            _ => panic!("Grammar of `table` changed without updating tree builder!")
        }
    }
    Ok(())
}

fn parse_short_string(pair: Pair<Rule>)-> Result<Rc<str>, LuaLikeError> {
    let mut buf = Vec::<u8>::new();
    for chunk in pair.into_inner() {
        match chunk.as_rule() {
            Rule::short_string_plain => buf.extend_from_slice(chunk.as_str().as_bytes()),
            Rule::string_esc_c => buf.push(match chunk.as_str() {
                "a" => 0x07,
                "b" => 0x08,
                "f" => 0x0C,
                "n" => 0x0A,
                "r" => 0x0D,
                "t" => 0x09,
                "v" => 0x0B,
                "\\" => 0x5C,
                "\"" => 0x22,
                "\'" => 0x27,
                _ => panic!("Somehow missed a C-like escape!")
            }),
            Rule::string_esc_hex => {
                let hex = &chunk.as_str()[1..];
                let hv = u8::from_str_radix(hex, 16).unwrap();
                buf.push(hv);
            },
            Rule::string_esc_dec => {
                let dec = chunk.as_str();
                let dv = u16::from_str_radix(dec, 10).unwrap();
                if dv > 255 { return Err(LuaLikeError::BadEscape) }
                buf.push(dv as u8);
            },
            Rule::string_esc_unicode => {
                let us = &chunk.as_str()[2..];
                let us = &us[..(us.len() - 1)];
                let cv = u32::from_str_radix(us, 16)
                    .ok()
                    .and_then(char::from_u32);
                match cv {
                    Some(c) => {
                        let mut b = [0u8; 4];
                        c.encode_utf8(&mut b);
                        buf.extend_from_slice(&b);
                    },
                    None => return Err(LuaLikeError::BadEscape)
                }
            },
            _ => panic!("Unexpected variation between the grammmar of `short_string` and its handling")
        }
    }
    match String::from_utf8(buf) {
        Ok(st) => Ok(st.into()),
        Err(_) => Err(LuaLikeError::BadStringEncoding),
    }
}

/*use logos::{Lexer, Logos};

#[derive(Logos, Debug, PartialEq)]
//#[logos(extras = LexExtras)]
enum Token {
    #[regex("[_[:alpha:]][_[:alpha:][:digit:]]*", lex_ident)]
    Ident(Rc<str>),

    #[regex(r#"["']"#, lex_short_string)]
    #[regex(r#"\[=*\["#, lex_long_string)]
    String(String),

    //#[regex(r"-?0[xX][0-9A-Fa-f]+(\.[0-9A-Fa-f]*)?([pP]-?[0-9]+)?", parse_hex_num)]
    #[regex(r"[-+]?[0-9]+(\.[0-9]*)?([eE][-+]?[0-9]+)?", parse_dec_num)]
    Number(f32),

    #[token("(")] LeftParen,
    #[token(")")] RightParen,
    #[token("{")] LeftBrace,
    #[token("}")] RightBrace,
    #[token("[")] LeftBracket,
    #[token("]")] RightBracket,
    #[token(",")] Comma,
    #[token("=")] Equals,

    #[regex("--.*[\r\n]", logos::skip)]
    #[regex(r"[ \r\n]+", logos::skip)]
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
    let buf = Vec::<u8>::new();
    let eos = lex.slice();
    let strlex = StringPart::lexer(lex.remainder());
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

fn parse_dec_num(lex: &mut Lexer<Token>) -> Result<f32, ()> {
    <f32 as std::str::FromStr>::from_str(lex.slice()).map_err(|_|())
}
*/

