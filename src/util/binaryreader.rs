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

pub struct NullTerminatedString;
impl ItemReader for NullTerminatedString {
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