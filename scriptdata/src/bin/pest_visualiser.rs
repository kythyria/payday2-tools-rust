//! Function to visualise Pest parse results.

use std::io::Write;
use std::io::Result;

use pest::*;
use pest::iterators::*;
use scriptdata::lua_like;

pub struct Visualiser<'a, O: Write> {
    out: &'a mut O,
    dent: Vec<&'static str>
}

impl<'a, O: Write> Visualiser<'a, O> {
    fn draw_pair<R: RuleType>(&mut self, p: &Pair<R>) -> Result<()> {
        writeln!(
            self.out, "{:?} {}..{} {:?}", p.as_rule(), p.as_span().start(), p.as_span().end(), p.as_str()
        )?;

        self.draw_pairs(p.clone().into_inner())
    }

    fn draw_pairs<R: RuleType>(&mut self, p: Pairs<R>) -> Result<()> {
        let mut children = p.peekable();
        while let Some(c) = children.next() {
            for n in &self.dent {
                write!(self.out, "{}", n)?;
            }
            if children.peek() != None {
                write!(self.out, "{}", "├─ ")?;
                self.dent.push("│  ");
            }
            else {
                write!(self.out, "{}", "└─ ")?;
                self.dent.push("   ");
            }
            self.draw_pair(&c)?;
            self.dent.pop();
        }
        Ok(())
    }
}

pub fn draw<R: RuleType, O: Write>(o: &mut O, p: Pairs<R>) -> Result<()> {
    Visualiser { out: o, dent: Vec::new() }.draw_pairs(p)
}

pub fn main() {
    let args = std::env::args().skip(1).next().unwrap();
    println!("File: {}\n", args);
    let input = std::fs::read_to_string(args).unwrap();
    match lua_like::get_parse(&input) {
        Ok(tree) => draw(&mut std::io::stdout(), tree).unwrap(),
        Err(e) => println!("{}", e)
    }
}