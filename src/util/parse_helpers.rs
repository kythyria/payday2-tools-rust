//! Helpers for parsing stuff by macro
//!
//! The macros in the macro crate assume that this is imported as `parse_helpers`.

use std::convert::TryInto;
use std::io::{Result as IoResult};
use std::io::{Write};

use nom::{IResult, sequence::Tuple};
use nom::bytes::complete::{is_not, tag, take_until};
use nom::combinator::{map, map_res};
use nom::multi::{fill, length_data, length_count};
use nom::number::complete::{le_u8, le_u16, le_u32, le_u64, le_i8, le_i16, le_i32, le_i64, le_f32, le_f64};
use nom::sequence::{tuple, terminated};
use pd2tools_macros::gen_tuple_parsers;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct InvalidDiscriminant {
    pub discriminant: u32
}

pub trait Parse where Self: Sized {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self>;
    fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()>;
}

macro_rules! simple_parse {
    ($t:ty, $parser:expr) => {
        impl Parse for $t {
            fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
                $parser(input)
            }
        
            fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
                output.write_all(&self.to_le_bytes())
            }
        }
    }
}

simple_parse!(u8, le_u8);
simple_parse!(u16, le_u16);
simple_parse!(u32, le_u32);
simple_parse!(u64, le_u64);
simple_parse!(i8, le_i8);
simple_parse!(i16, le_i16);
simple_parse!(i32, le_i32);
simple_parse!(i64, le_i64);
simple_parse!(f32, le_f32);
simple_parse!(f64, le_f64);

impl Parse for bool {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        map_res(le_u8, |i| match i {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(())
        })(input)
    }

    fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
        <u8 as Parse>::serialize(match self {
            true => &1, false => &0
        }, output)
    }
}

macro_rules! vek_parse {
    (@parser $discard:ident) => { <T as Parse>::parse };
    ($name:ident, $($field:ident),* ) => {
        impl<T: Parse> Parse for vek::$name<T> {
            fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
                let (rest, ( $($field),*) ) = tuple(( $(vek_parse!(@parser $field)),* ))(input)?;
                Ok((rest, vek::$name { $($field),* }))
            }
        
            fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
                $( self.$field.serialize(output)?; )*
                Ok(())
            }
        }
    }
}

vek_parse!(Vec4, x, y, z, w);
vek_parse!(Vec3, x, y, z);
vek_parse!(Vec2, x, y);
vek_parse!(Rgb, r, g, b);
vek_parse!(Rgba, r, g, b, a);

impl<T: Parse + Default> Parse for vek::Mat4<T> {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        let mut out: [T; 16] = Default::default();
        let (rest, ()) = fill(<T as Parse>::parse, &mut out)(input)?;
        Ok((rest, vek::Mat4::from_col_array(out)))
    }

    fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
        let buf = self.as_col_slice();
        for i in buf {
            i.serialize(output)?;
        }
        Ok(())
    }
}

impl Parse for String {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        let ts = terminated(take_until("\0"), tag(b"\0"));
        let tstr = map_res(ts, std::str::from_utf8);
        map(tstr, String::from)(input)
    }
    fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
        output.write_all(self.as_bytes())?;
        output.write_all(b"\0")
    }
}

impl Parse for crate::hashindex::Hash {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        map(le_u64, crate::hashindex::Hash)(input)
    }
    fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
        self.0.serialize(output)
    }
}

impl<T: Parse> Parse for Vec<T> {
    fn parse<'a>(input: &'a [u8]) -> IResult<&'a [u8], Self> {
        length_count(le_u32, <T as Parse>::parse)(input)
    }

    fn serialize<O: Write>(&self, output: &mut O) -> IoResult<()> {
        let count: u32 = self.len().try_into().map_err(|_| std::io::ErrorKind::InvalidInput)?;
        count.serialize(output)?;
        for i in self.iter() {
            i.serialize(output)?;
        }
        Ok(())
    }
}

gen_tuple_parsers!(16);

pub trait WireFormat<T> {
    fn parse_into<'a>(input: &'a [u8]) -> IResult<&'a [u8], T>;
    fn serialize_from<O: Write>(data: &T, output: &mut O) -> IoResult<()>;
}

impl<T: Parse> WireFormat<T> for T {
    fn parse_into<'a>(input: &'a [u8]) -> IResult<&'a [u8], T> {
        <T as Parse>::parse(input)
    }

    fn serialize_from<O: Write>(data: &T, output: &mut O) -> IoResult<()> {
        <T as Parse>::serialize(data, output)
    }
}

