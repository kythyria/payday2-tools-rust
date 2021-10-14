use thiserror::Error;
use logos::{Lexer, Logos};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unexpected token {0:?} at byte {1}")]
    BadToken(Token, usize),

    #[error("Bad escape \\{0}")]
    BadEscape(char),

    #[error("Unexpected end of file after {0} chars")]
    EarlyEof(usize)
}

#[derive(Logos, PartialEq, Eq, Debug)]
pub enum Token {
    #[token("{")] LeftBrace,
    #[token("}")] RightBrace,
    #[token("#include")] Include,
    #[token("#base")] Base,
    #[regex(r#"\[\P{White_Space}+\]"#, unenclose_conditional)] Conditional(String),

    #[regex(r#"[^"\p{White_Space}]+"#, |lex| String::from(lex.slice()))]
    #[regex(r#""([^"\\]|\\[ntvbrfa\\?'"])*""#, unescape)]
    Text(String),

    #[regex(r"//[^\n]\n", logos::skip)]
    #[regex(r#"\p{White_Space}+"#, logos::skip)]
    #[error]
    Error
}

fn unenclose_conditional(lex: &mut Lexer<Token>) -> String {
    let s = &lex.slice()[1..(lex.slice().len() - 1)];
    String::from(s)
}

fn unescape(lex: &mut Lexer<Token>) -> Option<String> {
    #[derive(Logos, Debug)]
    enum Ch {
        #[regex(r#"\\[ntvbrfa\\?'"]"#)] Escape,
        #[regex(r"[^\\]+")] Normal,
        #[error] Error,
    }

    let rawdata = &lex.slice()[1..(lex.slice().len() - 1)];
    let mut result = String::with_capacity(rawdata.len());
    let mut lexer = Ch::lexer(rawdata);
    while let Some(tok) = lexer.next() {
        match tok {
            Ch::Error => return None,
            Ch::Normal => result.push_str(lexer.slice()),
            Ch::Escape => result.push(match &lexer.slice()[1..2] {
                "n" => '\n',
                "t" => '\t',
                "v" => '\x0b',
                "b" => '\x08',
                "r" => '\r',
                "f" => '\x0c',
                "a" => '\x07',
                "\\" => '\\',
                "?" => '?',
                "\'" => '\'',
                "\"" => '\"',
                _ => return None
            })
        }
    }
    Some(result)
}

#[derive(Default, Debug)]
pub struct Node {
    pub name: String,
    pub condition: Option<String>,
    pub data: Data
}
impl Node {
    pub fn has_name(&self, name: &str) -> Option<&Self> {
        if self.name == name { Some(self) } else { None }
    }

    pub fn string_data(&self) -> Option<&str> {
        if let Data::String(s) = &self.data { Some(&s) } else { None }
    }

    pub fn section_data(&self) -> Option<&[Node]> {
        if let Data::Section(s) = &self.data { Some(s.as_slice()) } else { None }
    }
}

#[derive(Debug)]
pub enum Data {
    String(String),
    Section(Vec<Node>)
} 
impl Default for Data {
    fn default() -> Data { Data::String(String::new()) }
}

pub fn parse(input: &str) -> Result<Node,Error> {
    let mut tokens = Token::lexer(input);
    match parse_node(&mut tokens) {
        NodeParseResult::Node(n) => Ok(n),
        NodeParseResult::Err(e) => Err(e),
        NodeParseResult::GroupFinished => panic!("Shouldn't be returning GroupFinished for a complete node"),
    }
}

enum NodeParseResult {
    Node(Node),
    GroupFinished,
    Err(Error)
}
impl From<Error> for NodeParseResult {
    fn from(e: Error) -> Self { Self::Err(e) }
}

fn parse_node(tokens: &mut Lexer<Token>) -> NodeParseResult {
    let mut node = Node::default();

    let mut tok = tokens.next();
    match tok {
        Some(Token::RightBrace) => return NodeParseResult::GroupFinished,
        Some(Token::Conditional(c)) => {
            node.condition = Some(c);
            tok = tokens.next();
            match tok {
                Some(Token::Text(t)) => node.name = t,
                Some(t) => return Error::BadToken(t, tokens.span().start).into(),
                None => return Error::EarlyEof(tokens.span().end).into()
            }
       },
       Some(Token::Text(t)) => node.name = t,
       Some(t) => return Error::BadToken(t, tokens.span().start).into(),
       None => return Error::EarlyEof(tokens.span().end).into()
    }

    tok = tokens.next();
    match tok {
        Some(Token::Text(t)) => node.data = Data::String(t),
        Some(Token::LeftBrace) => {
            let mut data = Vec::<Node>::new();
            loop {
                match parse_node(tokens) {
                    NodeParseResult::Node(n) => data.push(n),
                    NodeParseResult::GroupFinished => break,
                    NodeParseResult::Err(e) => return NodeParseResult::Err(e),
                }
            }
            node.data = Data::Section(data);
        },
        Some(t) => return Error::BadToken(t, tokens.span().start).into(),
        None => return Error::EarlyEof(tokens.span().end).into()
    }
    
    NodeParseResult::Node(node)
}