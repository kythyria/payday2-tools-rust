use std::{io::{Read, Write, Error as IoError, BufRead}, marker::PhantomData, convert::TryInto, any::type_name};

use pd2tools_macros::tuple_itemreaders;

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("String contains invalid UTF-8 starting at character {0}")]
    BadUtf8(usize),

    #[error("{1} contains {0} items, too many to be counted with a {2}")]
    TooManyItems(usize, &'static str, &'static str),

    #[error("Unreasonably large item count for {0}")]
    BogusCount(&'static str),

    #[error("Bad conversion from {0} to {1}")]
    BadConvert(&'static str, &'static str),

    #[error("Unrecognised discriminant {1} in type {0}")]
    BadDiscriminant(&'static str, u128),

    #[error("Format constraint violation: {0}")]
    Schema(&'static str),

    #[error("Item claims to be {0} bytes long")]
    ItemTooLong(usize),

    #[error("IO error: {0}")]
    Io(#[from] IoError)
}
impl From<std::string::FromUtf8Error> for ReadError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Self::BadUtf8(e.utf8_error().valid_up_to())
    }
}
impl From<ReadError> for IoError {
    fn from(re: ReadError) -> Self {
        match re {
            ReadError::Io(e) => e,
            _ => IoError::new(std::io::ErrorKind::Other, re)
        }
    }
}

/// Defines how to read/write a `Item` from/to a stream. TODO: bytemuck integration.
pub trait ItemReader {
    type Error;
    type Item;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error>;
    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error>;
}

/// Extend a `Read` to be able to read objects, not just bytes.
pub trait ReadExt: Read + BufRead {
    fn read_item<I: ItemReader<Item=I>>(&mut self) -> Result<I, I::Error>;
    fn read_item_as<P: ItemReader>(&mut self) -> Result<P::Item, P::Error>;
}

pub trait WriteExt: Write {
    fn write_item<I: ItemReader<Item=I>>(&mut self, item: &I) -> Result<(), I::Error>;
    fn write_item_as<P: ItemReader>(&mut self, item: &P::Item) -> Result<(), P::Error>;
}

// https://discord.com/channels/273534239310479360/1009669096704573511

impl<T: Read + BufRead> ReadExt for T {
    fn read_item<I: ItemReader<Item=I>>(&mut self) -> Result<I, I::Error> {
        I::read_from_stream(self)
    }

    fn read_item_as<P: ItemReader>(&mut self) -> Result<P::Item, P::Error> {
        P::read_from_stream(self)
    }
}

impl<T: Write> WriteExt for T {
    fn write_item<I: ItemReader<Item=I>>(&mut self, item: &I) -> Result<(), I::Error> {
        I::write_to_stream(self, item)
    }

    fn write_item_as<P: ItemReader>(&mut self, item: &P::Item) -> Result<(), P::Error> {
        P::write_to_stream(self, item)
    }
}

macro_rules! numeric_itemreaders {
    ($($ty:ty),*) => { $(
        impl ItemReader for $ty {
            type Error = ReadError;
            type Item = Self;
        
            fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
                let mut buf = [0u8; std::mem::size_of::<$ty>()];
                stream.read_exact(&mut buf)?;
                Ok(<$ty>::from_le_bytes(buf))
            }
            fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), ReadError> {
                let mut buf = <$ty>::to_le_bytes(*item);
                Ok(stream.write_all(&mut buf)?)
            }
        }
    )*}
}

numeric_itemreaders!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);
tuple_itemreaders!(16);

impl ItemReader for String {
    type Error = ReadError;
    type Item = Self;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        stream.read_item_as::<CountedString<u32>>()
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        stream.write_item_as::<CountedString<u32>>(item)
    }
}

pub struct CountedString<TCount>(PhantomData<TCount>);
impl<TCount> ItemReader for CountedString<TCount>
where
    TCount: ItemReader<Error=ReadError>,
    TCount::Item: TryInto<usize>,
    usize: TryInto<TCount::Item>
{
    type Error = ReadError;
    type Item = String;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let bytes = CountedVec::<u8, TCount>::read_from_stream(stream)?;
        let res = String::from_utf8(bytes)?;
        Ok(res)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let wire_count: TCount::Item = match item.len().try_into() {
            Ok(c) => c,
            Err(_) => return Err(ReadError::TooManyItems(item.len(), type_name::<Self::Item>(), type_name::<TCount::Item>()))
        };
        stream.write_item_as::<TCount>(&wire_count)?;
        for i in item.as_bytes() {
            stream.write_item_as::<u8>(i)?;
        }
        Ok(())
    }
}

impl<T: ItemReader<Item=T>> ItemReader for Vec<T>
where
    ReadError: From<T::Error>
{
    type Error = ReadError;
    type Item = Self;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        CountedVec::<T, u32>::read_from_stream(stream)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        CountedVec::<T, u32>::write_to_stream(stream, item)
    }
}

pub struct CountedVec<TParser, TCount=u32>(PhantomData<TParser>, PhantomData<TCount>);
impl<TParser, TCount> ItemReader for CountedVec<TParser, TCount>
where
    TParser: ItemReader,
    TCount: ItemReader,
    TCount::Item: TryInto<usize>,
    usize: TryInto<TCount::Item>,
    ReadError: From<TParser::Error>,
    ReadError: From<TCount::Error>
{
    type Error = ReadError;
    type Item = Vec<TParser::Item>;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let count = match stream.read_item_as::<TCount>()?.try_into() {
            Ok(c) => c,
            Err(_) => return Err(ReadError::BogusCount(type_name::<Self::Item>()))
        };
        let mut res = Vec::<TParser::Item>::with_capacity(count);
        for _ in 0..count {
            res.push(stream.read_item_as::<TParser>()?);
        }
        Ok(res)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let wire_count = match item.len().try_into() {
            Ok(c) => c,
            Err(_) => return Err(ReadError::TooManyItems(item.len(), type_name::<Self::Item>(), type_name::<TCount>()))
        };
        stream.write_item_as::<TCount>(&wire_count)?;
        for i in item {
            stream.write_item_as::<TParser>(i)?;
        }
        Ok(())
    }
}

impl<T: ItemReader<Item=T> + Default, const C: usize> ItemReader for [T; C]
where
    T: ItemReader<Item=T> + Default,
    [T; C]: Default
{
    type Error = T::Error;
    type Item = [T; C];

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let mut buf: Self::Item = Default::default();
        for i in  0..C {
            buf[i] = stream.read_item()?;
        }
        Ok(buf)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        for i in  0..C {
            stream.write_item(&item[i])?;
        }
        Ok(())
    }
}

impl<T: ItemReader<Item=T> + Default> ItemReader for Box<[T]>
where
    T: ItemReader<Item=T,Error=ReadError> + Default,
    Box<[T]>: Default
{
    type Error = ReadError;
    type Item = Box<[T]>;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let v: Vec<T> = stream.read_item()?;
        Ok(v.into_boxed_slice())
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        for i in  0..item.len() {
            stream.write_item(&item[i])?;
        }
        Ok(())
    }
}

macro_rules! vek_itemreader {
    ($vekty:ident, $($field:ident),+) => {
        impl<T: ItemReader<Item=T>> ItemReader for vek::$vekty<T> {
            type Error = T::Error;
            type Item = Self;
        
            fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
                $(let $field = stream.read_item()?;)+
                Ok(Self::new($($field),+))
            }
        
            fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
                $(stream.write_item(&item.$field)?;)+
                Ok(())
            }
        }
    }
}

vek_itemreader!(Vec4, x, y, z, w);
vek_itemreader!(Vec3, x, y, z);
vek_itemreader!(Vec2, x, y);
vek_itemreader!(Rgb, r, g, b);
vek_itemreader!(Rgba, r, g, b, a);

pub struct Bgra<T>(PhantomData<T>);
impl<T: ItemReader<Item=T> + Default + Clone> ItemReader for Bgra<T> {
    type Error = T::Error;
    type Item = vek::Rgba<T>;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        Ok(stream.read_item::<Self::Item>()?.shuffled_bgra())
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let c = item.clone().shuffled_bgra();
        Ok(stream.write_item::<Self::Item>(&c)?)
    }
}

impl<T: ItemReader<Item=T> + Default + Clone> ItemReader for vek::Mat4<T> {
    type Error = T::Error;
    type Item = Self;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        Ok(Self::from_col_array(stream.read_item()?))
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        stream.write_item(&item.clone().into_col_array())
    }
}

impl ItemReader for bool {
    type Error = ReadError;
    type Item = bool;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let v: u8= stream.read_item_as::<u8>()?;
        match v {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ReadError::BadConvert("u8", "bool"))
        }
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        stream.write_item_as::<u8>(&item.clone().into())?;
        Ok(())
    }
}

pub struct NullTerminatedUtf8String;
impl ItemReader for NullTerminatedUtf8String {
    type Error = ReadError;
    type Item = String;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let mut data = Vec::<u8>::new();
        stream.read_until(0, &mut data)?;
        Ok(String::from_utf8(data)?)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        stream.write_all(item.as_bytes())?;
        stream.write_all(&[0])?;
        Ok(())
    }
}

pub struct NullTerminated1252String;
impl ItemReader for NullTerminated1252String {
    type Error = ReadError;
    type Item = String;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let mut data = Vec::<u8>::new();
        stream.read_until(0, &mut data)?;
        let data: String = data.into_iter().map(|i| match i {
            0x80 => '\u{20ac}', // Euro Sign
            0x82 => '\u{201a}', // Single Low-9 Quotation Mark
            0x83 => '\u{0192}', // Latin Small Letter F With Hook
            0x84 => '\u{201e}', // Double Low-9 Quotation Mark
            0x85 => '\u{2026}', // Horizontal Ellipsis
            0x86 => '\u{2020}', // Dagger
            0x87 => '\u{2021}', // Double Dagger
            0x88 => '\u{02c6}', // Modifier Letter Circumflex Accent
            0x89 => '\u{2030}', // Per Mille Sign
            0x8a => '\u{0160}', // Latin Capital Letter S With Caron
            0x8b => '\u{2039}', // Single Left-Pointing Angle Quotation Mark
            0x8c => '\u{0152}', // Latin Capital Ligature Oe
            0x8e => '\u{017d}', // Latin Capital Letter Z With Caron
            0x91 => '\u{2018}', // Left Single Quotation Mark
            0x92 => '\u{2019}', // Right Single Quotation Mark
            0x93 => '\u{201c}', // Left Double Quotation Mark
            0x94 => '\u{201d}', // Right Double Quotation Mark
            0x95 => '\u{2022}', // Bullet
            0x96 => '\u{2013}', // En Dash
            0x97 => '\u{2014}', // Em Dash
            0x98 => '\u{02dc}', // Small Tilde
            0x99 => '\u{2122}', // Trade Mark Sign
            0x9a => '\u{0161}', // Latin Small Letter S With Caron
            0x9b => '\u{203a}', // Single Right-Pointing Angle Quotation Mark
            0x9c => '\u{0153}', // Latin Small Ligature Oe
            0x9e => '\u{017e}', // Latin Small Letter Z With Caron
            0x9f => '\u{0178}', // Latin Capital Letter Y With Diaeresis
            i => i as char
        }).collect();
        Ok(data)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let bytes: Vec<u8> = item.chars().map(|c| match c {
            // No null, it's null-terminated
            '\u{0001}' => 0x01u8, // Start Of Heading
            '\u{0002}' => 0x02u8, // Start Of Text
            '\u{0003}' => 0x03u8, // End Of Text
            '\u{0004}' => 0x04u8, // End Of Transmission
            '\u{0005}' => 0x05u8, // Enquiry
            '\u{0006}' => 0x06u8, // Acknowledge
            '\u{0007}' => 0x07u8, // Bell
            '\u{0008}' => 0x08u8, // Backspace
            '\u{0009}' => 0x09u8, // Horizontal Tabulation
            '\u{000a}' => 0x0au8, // Line Feed
            '\u{000b}' => 0x0bu8, // Vertical Tabulation
            '\u{000c}' => 0x0cu8, // Form Feed
            '\u{000d}' => 0x0du8, // Carriage Return
            '\u{000e}' => 0x0eu8, // Shift Out
            '\u{000f}' => 0x0fu8, // Shift In
            '\u{0010}' => 0x10u8, // Data Link Escape
            '\u{0011}' => 0x11u8, // Device Control One
            '\u{0012}' => 0x12u8, // Device Control Two
            '\u{0013}' => 0x13u8, // Device Control Three
            '\u{0014}' => 0x14u8, // Device Control Four
            '\u{0015}' => 0x15u8, // Negative Acknowledge
            '\u{0016}' => 0x16u8, // Synchronous Idle
            '\u{0017}' => 0x17u8, // End Of Transmission Block
            '\u{0018}' => 0x18u8, // Cancel
            '\u{0019}' => 0x19u8, // End Of Medium
            '\u{001a}' => 0x1au8, // Substitute
            '\u{001b}' => 0x1bu8, // Escape
            '\u{001c}' => 0x1cu8, // File Separator
            '\u{001d}' => 0x1du8, // Group Separator
            '\u{001e}' => 0x1eu8, // Record Separator
            '\u{001f}' => 0x1fu8, // Unit Separator
            '\u{0020}' => 0x20u8, // Space
            '\u{0021}' => 0x21u8, // Exclamation Mark
            '\u{0022}' => 0x22u8, // Quotation Mark
            '\u{0023}' => 0x23u8, // Number Sign
            '\u{0024}' => 0x24u8, // Dollar Sign
            '\u{0025}' => 0x25u8, // Percent Sign
            '\u{0026}' => 0x26u8, // Ampersand
            '\u{0027}' => 0x27u8, // Apostrophe
            '\u{0028}' => 0x28u8, // Left Parenthesis
            '\u{0029}' => 0x29u8, // Right Parenthesis
            '\u{002a}' => 0x2au8, // Asterisk
            '\u{002b}' => 0x2bu8, // Plus Sign
            '\u{002c}' => 0x2cu8, // Comma
            '\u{002d}' => 0x2du8, // Hyphen-Minus
            '\u{002e}' => 0x2eu8, // Full Stop
            '\u{002f}' => 0x2fu8, // Solidus
            '\u{0030}' => 0x30u8, // Digit Zero
            '\u{0031}' => 0x31u8, // Digit One
            '\u{0032}' => 0x32u8, // Digit Two
            '\u{0033}' => 0x33u8, // Digit Three
            '\u{0034}' => 0x34u8, // Digit Four
            '\u{0035}' => 0x35u8, // Digit Five
            '\u{0036}' => 0x36u8, // Digit Six
            '\u{0037}' => 0x37u8, // Digit Seven
            '\u{0038}' => 0x38u8, // Digit Eight
            '\u{0039}' => 0x39u8, // Digit Nine
            '\u{003a}' => 0x3au8, // Colon
            '\u{003b}' => 0x3bu8, // Semicolon
            '\u{003c}' => 0x3cu8, // Less-Than Sign
            '\u{003d}' => 0x3du8, // Equals Sign
            '\u{003e}' => 0x3eu8, // Greater-Than Sign
            '\u{003f}' => 0x3fu8, // Question Mark
            '\u{0040}' => 0x40u8, // Commercial At
            '\u{0041}' => 0x41u8, // Latin Capital Letter A
            '\u{0042}' => 0x42u8, // Latin Capital Letter B
            '\u{0043}' => 0x43u8, // Latin Capital Letter C
            '\u{0044}' => 0x44u8, // Latin Capital Letter D
            '\u{0045}' => 0x45u8, // Latin Capital Letter E
            '\u{0046}' => 0x46u8, // Latin Capital Letter F
            '\u{0047}' => 0x47u8, // Latin Capital Letter G
            '\u{0048}' => 0x48u8, // Latin Capital Letter H
            '\u{0049}' => 0x49u8, // Latin Capital Letter I
            '\u{004a}' => 0x4au8, // Latin Capital Letter J
            '\u{004b}' => 0x4bu8, // Latin Capital Letter K
            '\u{004c}' => 0x4cu8, // Latin Capital Letter L
            '\u{004d}' => 0x4du8, // Latin Capital Letter M
            '\u{004e}' => 0x4eu8, // Latin Capital Letter N
            '\u{004f}' => 0x4fu8, // Latin Capital Letter O
            '\u{0050}' => 0x50u8, // Latin Capital Letter P
            '\u{0051}' => 0x51u8, // Latin Capital Letter Q
            '\u{0052}' => 0x52u8, // Latin Capital Letter R
            '\u{0053}' => 0x53u8, // Latin Capital Letter S
            '\u{0054}' => 0x54u8, // Latin Capital Letter T
            '\u{0055}' => 0x55u8, // Latin Capital Letter U
            '\u{0056}' => 0x56u8, // Latin Capital Letter V
            '\u{0057}' => 0x57u8, // Latin Capital Letter W
            '\u{0058}' => 0x58u8, // Latin Capital Letter X
            '\u{0059}' => 0x59u8, // Latin Capital Letter Y
            '\u{005a}' => 0x5au8, // Latin Capital Letter Z
            '\u{005b}' => 0x5bu8, // Left Square Bracket
            '\u{005c}' => 0x5cu8, // Reverse Solidus
            '\u{005d}' => 0x5du8, // Right Square Bracket
            '\u{005e}' => 0x5eu8, // Circumflex Accent
            '\u{005f}' => 0x5fu8, // Low Line
            '\u{0060}' => 0x60u8, // Grave Accent
            '\u{0061}' => 0x61u8, // Latin Small Letter A
            '\u{0062}' => 0x62u8, // Latin Small Letter B
            '\u{0063}' => 0x63u8, // Latin Small Letter C
            '\u{0064}' => 0x64u8, // Latin Small Letter D
            '\u{0065}' => 0x65u8, // Latin Small Letter E
            '\u{0066}' => 0x66u8, // Latin Small Letter F
            '\u{0067}' => 0x67u8, // Latin Small Letter G
            '\u{0068}' => 0x68u8, // Latin Small Letter H
            '\u{0069}' => 0x69u8, // Latin Small Letter I
            '\u{006a}' => 0x6au8, // Latin Small Letter J
            '\u{006b}' => 0x6bu8, // Latin Small Letter K
            '\u{006c}' => 0x6cu8, // Latin Small Letter L
            '\u{006d}' => 0x6du8, // Latin Small Letter M
            '\u{006e}' => 0x6eu8, // Latin Small Letter N
            '\u{006f}' => 0x6fu8, // Latin Small Letter O
            '\u{0070}' => 0x70u8, // Latin Small Letter P
            '\u{0071}' => 0x71u8, // Latin Small Letter Q
            '\u{0072}' => 0x72u8, // Latin Small Letter R
            '\u{0073}' => 0x73u8, // Latin Small Letter S
            '\u{0074}' => 0x74u8, // Latin Small Letter T
            '\u{0075}' => 0x75u8, // Latin Small Letter U
            '\u{0076}' => 0x76u8, // Latin Small Letter V
            '\u{0077}' => 0x77u8, // Latin Small Letter W
            '\u{0078}' => 0x78u8, // Latin Small Letter X
            '\u{0079}' => 0x79u8, // Latin Small Letter Y
            '\u{007a}' => 0x7au8, // Latin Small Letter Z
            '\u{007b}' => 0x7bu8, // Left Curly Bracket
            '\u{007c}' => 0x7cu8, // Vertical Line
            '\u{007d}' => 0x7du8, // Right Curly Bracket
            '\u{007e}' => 0x7eu8, // Tilde
            '\u{007f}' => 0x7fu8, // Delete
            '\u{20ac}' => 0x80u8, // Euro Sign
            '\u{0081}' => 0x81u8, // 
            '\u{201a}' => 0x82u8, // Single Low-9 Quotation Mark
            '\u{0192}' => 0x83u8, // Latin Small Letter F With Hook
            '\u{201e}' => 0x84u8, // Double Low-9 Quotation Mark
            '\u{2026}' => 0x85u8, // Horizontal Ellipsis
            '\u{2020}' => 0x86u8, // Dagger
            '\u{2021}' => 0x87u8, // Double Dagger
            '\u{02c6}' => 0x88u8, // Modifier Letter Circumflex Accent
            '\u{2030}' => 0x89u8, // Per Mille Sign
            '\u{0160}' => 0x8au8, // Latin Capital Letter S With Caron
            '\u{2039}' => 0x8bu8, // Single Left-Pointing Angle Quotation Mark
            '\u{0152}' => 0x8cu8, // Latin Capital Ligature Oe
            '\u{008d}' => 0x8du8, // 
            '\u{017d}' => 0x8eu8, // Latin Capital Letter Z With Caron
            '\u{008f}' => 0x8fu8, // 
            '\u{0090}' => 0x90u8, // 
            '\u{2018}' => 0x91u8, // Left Single Quotation Mark
            '\u{2019}' => 0x92u8, // Right Single Quotation Mark
            '\u{201c}' => 0x93u8, // Left Double Quotation Mark
            '\u{201d}' => 0x94u8, // Right Double Quotation Mark
            '\u{2022}' => 0x95u8, // Bullet
            '\u{2013}' => 0x96u8, // En Dash
            '\u{2014}' => 0x97u8, // Em Dash
            '\u{02dc}' => 0x98u8, // Small Tilde
            '\u{2122}' => 0x99u8, // Trade Mark Sign
            '\u{0161}' => 0x9au8, // Latin Small Letter S With Caron
            '\u{203a}' => 0x9bu8, // Single Right-Pointing Angle Quotation Mark
            '\u{0153}' => 0x9cu8, // Latin Small Ligature Oe
            '\u{009d}' => 0x9du8, // 
            '\u{017e}' => 0x9eu8, // Latin Small Letter Z With Caron
            '\u{0178}' => 0x9fu8, // Latin Capital Letter Y With Diaeresis
            '\u{00a0}' => 0xa0u8, // No-Break Space
            '\u{00a1}' => 0xa1u8, // Inverted Exclamation Mark
            '\u{00a2}' => 0xa2u8, // Cent Sign
            '\u{00a3}' => 0xa3u8, // Pound Sign
            '\u{00a4}' => 0xa4u8, // Currency Sign
            '\u{00a5}' => 0xa5u8, // Yen Sign
            '\u{00a6}' => 0xa6u8, // Broken Bar
            '\u{00a7}' => 0xa7u8, // Section Sign
            '\u{00a8}' => 0xa8u8, // Diaeresis
            '\u{00a9}' => 0xa9u8, // Copyright Sign
            '\u{00aa}' => 0xaau8, // Feminine Ordinal Indicator
            '\u{00ab}' => 0xabu8, // Left-Pointing Double Angle Quotation Mark
            '\u{00ac}' => 0xacu8, // Not Sign
            '\u{00ad}' => 0xadu8, // Soft Hyphen
            '\u{00ae}' => 0xaeu8, // Registered Sign
            '\u{00af}' => 0xafu8, // Macron
            '\u{00b0}' => 0xb0u8, // Degree Sign
            '\u{00b1}' => 0xb1u8, // Plus-Minus Sign
            '\u{00b2}' => 0xb2u8, // Superscript Two
            '\u{00b3}' => 0xb3u8, // Superscript Three
            '\u{00b4}' => 0xb4u8, // Acute Accent
            '\u{00b5}' => 0xb5u8, // Micro Sign
            '\u{00b6}' => 0xb6u8, // Pilcrow Sign
            '\u{00b7}' => 0xb7u8, // Middle Dot
            '\u{00b8}' => 0xb8u8, // Cedilla
            '\u{00b9}' => 0xb9u8, // Superscript One
            '\u{00ba}' => 0xbau8, // Masculine Ordinal Indicator
            '\u{00bb}' => 0xbbu8, // Right-Pointing Double Angle Quotation Mark
            '\u{00bc}' => 0xbcu8, // Vulgar Fraction One Quarter
            '\u{00bd}' => 0xbdu8, // Vulgar Fraction One Half
            '\u{00be}' => 0xbeu8, // Vulgar Fraction Three Quarters
            '\u{00bf}' => 0xbfu8, // Inverted Question Mark
            '\u{00c0}' => 0xc0u8, // Latin Capital Letter A With Grave
            '\u{00c1}' => 0xc1u8, // Latin Capital Letter A With Acute
            '\u{00c2}' => 0xc2u8, // Latin Capital Letter A With Circumflex
            '\u{00c3}' => 0xc3u8, // Latin Capital Letter A With Tilde
            '\u{00c4}' => 0xc4u8, // Latin Capital Letter A With Diaeresis
            '\u{00c5}' => 0xc5u8, // Latin Capital Letter A With Ring Above
            '\u{00c6}' => 0xc6u8, // Latin Capital Ligature Ae
            '\u{00c7}' => 0xc7u8, // Latin Capital Letter C With Cedilla
            '\u{00c8}' => 0xc8u8, // Latin Capital Letter E With Grave
            '\u{00c9}' => 0xc9u8, // Latin Capital Letter E With Acute
            '\u{00ca}' => 0xcau8, // Latin Capital Letter E With Circumflex
            '\u{00cb}' => 0xcbu8, // Latin Capital Letter E With Diaeresis
            '\u{00cc}' => 0xccu8, // Latin Capital Letter I With Grave
            '\u{00cd}' => 0xcdu8, // Latin Capital Letter I With Acute
            '\u{00ce}' => 0xceu8, // Latin Capital Letter I With Circumflex
            '\u{00cf}' => 0xcfu8, // Latin Capital Letter I With Diaeresis
            '\u{00d0}' => 0xd0u8, // Latin Capital Letter Eth
            '\u{00d1}' => 0xd1u8, // Latin Capital Letter N With Tilde
            '\u{00d2}' => 0xd2u8, // Latin Capital Letter O With Grave
            '\u{00d3}' => 0xd3u8, // Latin Capital Letter O With Acute
            '\u{00d4}' => 0xd4u8, // Latin Capital Letter O With Circumflex
            '\u{00d5}' => 0xd5u8, // Latin Capital Letter O With Tilde
            '\u{00d6}' => 0xd6u8, // Latin Capital Letter O With Diaeresis
            '\u{00d7}' => 0xd7u8, // Multiplication Sign
            '\u{00d8}' => 0xd8u8, // Latin Capital Letter O With Stroke
            '\u{00d9}' => 0xd9u8, // Latin Capital Letter U With Grave
            '\u{00da}' => 0xdau8, // Latin Capital Letter U With Acute
            '\u{00db}' => 0xdbu8, // Latin Capital Letter U With Circumflex
            '\u{00dc}' => 0xdcu8, // Latin Capital Letter U With Diaeresis
            '\u{00dd}' => 0xddu8, // Latin Capital Letter Y With Acute
            '\u{00de}' => 0xdeu8, // Latin Capital Letter Thorn
            '\u{00df}' => 0xdfu8, // Latin Small Letter Sharp S
            '\u{00e0}' => 0xe0u8, // Latin Small Letter A With Grave
            '\u{00e1}' => 0xe1u8, // Latin Small Letter A With Acute
            '\u{00e2}' => 0xe2u8, // Latin Small Letter A With Circumflex
            '\u{00e3}' => 0xe3u8, // Latin Small Letter A With Tilde
            '\u{00e4}' => 0xe4u8, // Latin Small Letter A With Diaeresis
            '\u{00e5}' => 0xe5u8, // Latin Small Letter A With Ring Above
            '\u{00e6}' => 0xe6u8, // Latin Small Ligature Ae
            '\u{00e7}' => 0xe7u8, // Latin Small Letter C With Cedilla
            '\u{00e8}' => 0xe8u8, // Latin Small Letter E With Grave
            '\u{00e9}' => 0xe9u8, // Latin Small Letter E With Acute
            '\u{00ea}' => 0xeau8, // Latin Small Letter E With Circumflex
            '\u{00eb}' => 0xebu8, // Latin Small Letter E With Diaeresis
            '\u{00ec}' => 0xecu8, // Latin Small Letter I With Grave
            '\u{00ed}' => 0xedu8, // Latin Small Letter I With Acute
            '\u{00ee}' => 0xeeu8, // Latin Small Letter I With Circumflex
            '\u{00ef}' => 0xefu8, // Latin Small Letter I With Diaeresis
            '\u{00f0}' => 0xf0u8, // Latin Small Letter Eth
            '\u{00f1}' => 0xf1u8, // Latin Small Letter N With Tilde
            '\u{00f2}' => 0xf2u8, // Latin Small Letter O With Grave
            '\u{00f3}' => 0xf3u8, // Latin Small Letter O With Acute
            '\u{00f4}' => 0xf4u8, // Latin Small Letter O With Circumflex
            '\u{00f5}' => 0xf5u8, // Latin Small Letter O With Tilde
            '\u{00f6}' => 0xf6u8, // Latin Small Letter O With Diaeresis
            '\u{00f7}' => 0xf7u8, // Division Sign
            '\u{00f8}' => 0xf8u8, // Latin Small Letter O With Stroke
            '\u{00f9}' => 0xf9u8, // Latin Small Letter U With Grave
            '\u{00fa}' => 0xfau8, // Latin Small Letter U With Acute
            '\u{00fb}' => 0xfbu8, // Latin Small Letter U With Circumflex
            '\u{00fc}' => 0xfcu8, // Latin Small Letter U With Diaeresis
            '\u{00fd}' => 0xfdu8, // Latin Small Letter Y With Acute
            '\u{00fe}' => 0xfeu8, // Latin Small Letter Thorn
            '\u{00ff}' => 0xffu8, // Latin Small Letter Y With Diaeresis
            _ => b'_'
        }).collect();
        stream.write_all(&bytes)?;
        stream.write_all(&[0])?;
        Ok(())
    }
}