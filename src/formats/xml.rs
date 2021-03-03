use std::borrow::Cow;

use nom::{IResult, branch::alt, bytes::complete::{is_not, take_until, take_while, take_while1}, character::complete::digit1, combinator::{flat_map, map, map_opt, success}, error::{VerboseError, context}, sequence::{delimited, preceded, separated_pair, tuple}};
use nom::bytes::complete::{tag};
use nom::character::complete::{hex_digit1};

pub enum Token<'a> {
    Text(Cow<'a, str>),
    StartElement(Cow<'a, str>),
    Attribute(Cow<'a, str>, Cow<'a, str>),
    StartBody(Cow<'a, str>),
    EndElement(Cow<'a, str>),
    ShorthandEndElement,
    Comment(Cow<'a, str>),
    ProcessingInstruction(Cow<'a, str>, Cow<'a, str>),
    Error
}

/// Extremely tolerant XML tokeniser
///
/// This does not care about misnesting, duplicate attributes, spaces
/// between attributes, quoting attributes that don't have spaces in,
/// comments with hyphens in, multiple nodes at the root level, etc.
struct ForgivingTokeniser<'a> {
    src: &'a str,
    current_index: usize
}

fn nom_comment<'a>(input: &'a str) -> IResult<&str, Token<'a>, VerboseError<&str>> {
    let chomped = context("Comment", delimited(
        tag("<!--"),
        take_until("-->"),
        tag("-->")
    ))(input);
    let res = match chomped {
        Ok((i, c)) => Ok((i, Token::Comment(Cow::from(c)))),
        Err(c) => Err(c)
    };
    res
}

fn nom_cdata<'a>(input: &'a str) -> IResult<&str, Token<'a>, VerboseError<&str>> {
    let chomped = context("CDATA", delimited(
        tag("<![CDATA["),
        take_until("]]>"),
        tag("]]>")
    ))(input);
    match chomped {
        Ok((i, c)) => Ok((i, Token::Text(Cow::from(c)))),
        Err(c) => Err(c)
    }
}

fn nom_pi<'a>(input: &'a str) -> IResult<&str, Token<'a>, VerboseError<&str>> {
    let chomped = context("Processing Instruction", delimited(
        tag("<?"),
        separated_pair(
            take_until(" "),
            tag(" "),
            take_until("?>")
        ),
        tag("?>")
    ))(input);
    chomped.map(|(i,c)| (i, Token::ProcessingInstruction(Cow::from(c.0), Cow::from(c.1))))
}

fn map_tag<'a, R: Clone>(m: &'a str, result: R) -> impl FnMut(&'a str) -> IResult<&'a str, R, VerboseError<&str>> {
    preceded(tag(m), success(result))
}

fn hex_to_cow(input: &str) -> Option<Cow<'_, str>> {
    let num = u32::from_str_radix(input,16);
    num.ok().and_then(std::char::from_u32).map(|i| Cow::from(i.to_string()))
}

fn dec_to_cow(input: &str) -> Option<Cow<'_, str>> {
    let num = u32::from_str_radix(input,10);
    num.ok().and_then(std::char::from_u32).map(|i| Cow::from(i.to_string()))
}

fn nom_entity<'a>(input: &'a str) -> IResult<&str, Token, VerboseError<&str>> {
    context("Entity", delimited(
        tag("&"),
        alt((
            map_tag("lt", Cow::from("<")),
            map_tag("gt", Cow::from(">")),
            map_tag("apos", Cow::from("\'")),
            map_tag("quot", Cow::from("\"")),
            map_tag("amp", Cow::from("&")),
            map_opt(preceded(tag("#x"), hex_digit1), hex_to_cow),
            map_opt(preceded(tag("#"), digit1), dec_to_cow)
        )),
        tag(";")
    ))(input).map(|(i,o)| (i, Token::Text(o)))
}

fn nom_endelement<'a>(input: &str) -> IResult<&str, Token, VerboseError<&str>> {
    context("End Tag", delimited(
        tag("</"),
        take_until(">"),
        tag(">")
    ))(input).map(|(i,o)|(i, Token::EndElement(Cow::from(o))))
}

fn nom_startelement<'a>(input: &str) -> IResult<&str, Token, VerboseError<&str>> {
    context("Start Tag", preceded(
        tag("<"),
        is_not(" \r\n\t>/")
    ))(input).map(|(i,o)|(i, Token::StartElement(Cow::from(o))))
}

fn nom_endstarttag(input: &str)-> IResult<&str, bool, VerboseError<&str>> {
    context("End of start tag", alt((
        map(tag("/>"), |_| false),
        map(tag(">"), |_| true)
    )))(input)
}
fn nom_attribute(input: &str) -> IResult<&str, Token, VerboseError<&str>> {
    context("Attribute", preceded(
        take_while(is_whitespace),
        separated_pair(
            take_until("="),
            tag("="),
            nom_rcdata
        )
    ))(input).map(|(i, (n,v))| (i, Token::Attribute(Cow::from(n), v)))
}

fn is_whitespace(c: char) -> bool {
    unimplemented!()
}

fn nom_rcdata<'a>(input: &'a str) -> IResult<&str, Cow<'a, str>, VerboseError<&str>> {
    unimplemented!()
}

fn mode_text() {
    let options = alt((
        nom_endelement,
        nom_startelement,
        nom_comment,
        nom_cdata,
        nom_pi,
        nom_entity,
        //nom_text
    ));
}