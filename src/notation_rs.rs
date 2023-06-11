//! Common data notation for save files and scriptdata
//! 
//! Saves don't have a textual repr at all, and the vanilla ones for scriptdata suck for hand-viewing.
//! So this is an alternative.
//! 
//! Currently there's no way to represent NaN or Inf. 

use std::fmt::Write;

use proc_macro2::Span;
use syn::{Ident, LitInt, LitFloat, LitStr, LitByteStr, Lifetime, punctuated::Punctuated, Result as SyResult, token, Token};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::parse::discouraged::AnyDelimiter;
use proc_macro2::Delimiter;

pub enum Item {
    Integer(LitInt),
    Float(LitFloat),
    String(LitStr),
    Binary(LitByteStr),
    Bare(Ident),
    Reference(Lifetime),
    Compound(Compound),
}

pub struct Compound {
    pub ref_id: Option<Lifetime>,
    pub tag: Option<Ident>,
    //pub delim_span: DelimSpan,
    pub delimiter: Delimiter,
    pub body: Punctuated<CompoundEntry, token::Comma>,
}

pub enum CompoundEntry {
    Named(Item, Item),
    BareNamed(Ident, Item),
    Indexed(Item)
}

impl Parse for Item {
    fn parse(input: ParseStream) -> SyResult<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(LitInt) {
            input.parse().map(Item::Integer)
        }
        else if lookahead.peek(LitFloat) {
            input.parse().map(Item::Float)
        }
        else if lookahead.peek(LitStr) {
            input.parse().map(Item::String)
        }
        else if lookahead.peek(LitByteStr) {
            input.parse().map(Item::Binary)
        }
        else if lookahead.peek(Token![#]) {
            let _: Token![#] = input.parse()?;
            input.parse().map(Item::Bare)
        }
        else if lookahead.peek(Ident::peek_any) {
            input.parse().map(Item::Compound)
        }
        else if lookahead.peek(token::Brace) {
            input.parse().map(Item::Compound)
        }
        else if lookahead.peek(token::Bracket) {
            input.parse().map(Item::Compound)
        }
        else if lookahead.peek(token::Paren) {
            input.parse().map(Item::Compound)
        }
        else if lookahead.peek(Lifetime) {
            // This could be a reference or a named compound
            if input.peek2(Ident::peek_any) || input.peek2(token::Brace) || input.peek2(token::Bracket) || input.peek2(token::Paren) {
                input.parse().map(Item::Compound)
            }
            else {
                input.parse().map(Item::Reference)
            }
        }
        else {
            Err(lookahead.error())
        }
    }
}

impl Parse for Compound {
    fn parse(input: ParseStream) -> SyResult<Self> {
        let ref_id = if input.peek(Lifetime) {
            input.parse()?
        }
        else {
            None
        };

        let tag = if input.peek(Ident::peek_any) {
            Some(input.call(Ident::parse_any)?)
        }
        else {
            None
        };

        let (delimiter, _delim_span, content) = input.parse_any_delimiter()?;

        let body = content.call(Punctuated::parse_terminated)?;

        Ok(Compound { ref_id, tag, /*delim_span,*/ delimiter , body })
    }
}

impl Parse for CompoundEntry {
    fn parse(input: ParseStream) -> SyResult<Self> {
        if input.peek(Ident::peek_any) && input.peek2(token::Colon) {
            let name: Ident = input.call(Ident::parse_any)?;
            let _colon: token::Colon = input.parse()?;
            let value: Item = input.parse()?;
            return Ok(CompoundEntry::BareNamed(name, value));
        }
        
        let first: Item = input.parse()?;
        if input.peek(token::Colon) {
            let _colon: token::Colon = input.parse()?;
            let value: Item = input.parse()?;
            return Ok(CompoundEntry::Named(first, value))
        }
        else {
            return Ok(CompoundEntry::Indexed(first))
        }
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Integer(t) => t.fmt(f),
            Item::Float(t) => t.fmt(f),
            Item::String(t) => t.token().fmt(f),
            Item::Binary(t) => t.token().fmt(f),
            Item::Bare(b) => write!(f, "#{}", b),
            Item::Reference(t) => t.fmt(f),
            Item::Compound(t) => t.fmt(f),
        }
    }
}

impl std::fmt::Display for Compound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.ref_id {
            Some(r) => write!(f, "{} ", r)?,
            None => ()
        }

        match &self.tag {
            Some(r) => write!(f, "{} ", r)?,
            None => ()
        }

        match self.delimiter {
            Delimiter::Parenthesis => f.write_char('(')?,
            Delimiter::Brace => f.write_char('{')?,
            Delimiter::Bracket => f.write_char('[')?,
            Delimiter::None => (),
        }
        if !f.alternate() && self.body.len() > 1 {
            f.write_char('\n')?;
        }

        for pair in self.body.pairs() {
            match pair {
                syn::punctuated::Pair::Punctuated(i, _) => write!(f,"{},", i)?,
                syn::punctuated::Pair::End(i) => i.fmt(f)?,
            };
            f.write_char(if f.alternate() && self.body.len() > 1 {' '} else {'\n'})?;
        }

        match self.delimiter {
            Delimiter::Parenthesis => f.write_char(')')?,
            Delimiter::Brace => f.write_char('}')?,
            Delimiter::Bracket => f.write_char(']')?,
            Delimiter::None => (),
        }

        Ok(())
    }
}

impl std::fmt::Display for CompoundEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompoundEntry::Named(n, v) => write!(f, "{}: {}", n, v),
            CompoundEntry::BareNamed(n, v) => write!(f, "{}: {}", n, v),
            CompoundEntry::Indexed(v) => v.fmt(f),
        }
    }
}

impl Item {
    pub fn new_string(val: &str) -> Self {
        Item::String(LitStr::new(val, Span::call_site()))
    }

    pub fn new_binary(val: &[u8]) -> Self {
        Item::Binary(LitByteStr::new(val, Span::call_site()))
    }

    pub fn new_float(val: f32) -> Self {
        Item::Float(LitFloat::from(proc_macro2::Literal::f32_suffixed(val)))
    }

    pub fn new_i8(val: i8) -> Self {
        Item::Integer(LitInt::from(proc_macro2::Literal::i8_suffixed(val)))
    }

    pub fn new_i16(val: i16) -> Self {
        Item::Integer(LitInt::from(proc_macro2::Literal::i16_suffixed(val)))
    }

    pub fn new_u8(val: u8) -> Self {
        Item::Integer(LitInt::from(proc_macro2::Literal::u8_suffixed(val)))
    }

    pub fn new_u16(val: u16) -> Self {
        Item::Integer(LitInt::from(proc_macro2::Literal::u16_suffixed(val)))
    }

    pub fn new_bare(ident: &str) -> Self {
        Item::Bare(Ident::new(ident, Span::call_site()))
    }

    pub fn new_integer(int: isize) -> Self {
        Item::Integer(LitInt::from(proc_macro2::Literal::isize_unsuffixed(int)))
    }
}

impl Compound {
    pub fn new_braced() -> Self {
        Compound {
            ref_id: None,
            tag: None,
            delimiter: proc_macro2::Delimiter::Brace,
            body: syn::punctuated::Punctuated::new(),
        }
    }

    pub fn new_parenthesized() -> Self {
        Compound {
            ref_id: None,
            tag: None,
            delimiter: proc_macro2::Delimiter::Parenthesis,
            body: syn::punctuated::Punctuated::new(),
        }
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tag = Some(Ident::new(tag, Span::call_site()));
        self
    }

    pub fn push_bare(&mut self, name: &str, value: Item) -> &mut Self {
        let bare_ident = Ident::new(name, Span::call_site());
        self.body.push(CompoundEntry::BareNamed(bare_ident, value));
        self
    }

    pub fn push(&mut self, name: Item, value: Item) -> &mut Self {
        self.body.push(CompoundEntry::Named(name, value));
        self
    }

    pub fn push_indexed(&mut self, value: Item) -> &mut Self {
        self.body.push(CompoundEntry::Indexed(value));
        self
    }
}