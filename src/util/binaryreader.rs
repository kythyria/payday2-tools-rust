use std::{io::{Read, Write, Error as IoError}, marker::PhantomData, convert::TryInto, any::type_name};

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

    #[error("IO error: {0}")]
    Io(#[from] IoError)
}
impl From<std::string::FromUtf8Error> for ReadError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Self::BadUtf8(e.utf8_error().valid_up_to())
    }
}

/// Defines how to read/write a `T` from/to a stream. TODO: bytemuck integration.
pub trait ItemReader {
    type Error;
    type Item;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error>;
    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error>;
}

/// Extend a `Read` to be able to read objects, not just bytes.
pub trait ReadExt: Read {
    fn read_item<I: ItemReader<Item=I>>(&mut self) -> Result<I, I::Error>;
    fn read_item_as<P: ItemReader>(&mut self) -> Result<P::Item, P::Error>;
}

pub trait WriteExt: Write {
    fn write_item<I: ItemReader<Item=I>>(&mut self, item: &I) -> Result<(), I::Error>;
    fn write_item_as<P: ItemReader>(&mut self, item: &P::Item) -> Result<(), P::Error>;
}

// https://discord.com/channels/273534239310479360/1009669096704573511

impl<T: Read> ReadExt for T {
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
            type Error = IoError;
            type Item = Self;
        
            fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
                let mut buf = [0u8; std::mem::size_of::<$ty>()];
                stream.read_exact(&mut buf)?;
                Ok(<$ty>::from_le_bytes(buf))
            }
            fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), IoError> {
                let mut buf = <$ty>::to_le_bytes(*item);
                stream.write_all(&mut buf)
            }
        }
    )*}
}

numeric_itemreaders!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);

impl ItemReader for String {
    type Error = ReadError;
    type Item = Self;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let bytes = CountedVec::<u8, u32>::read_from_stream(stream)?;
        let res = String::from_utf8(bytes)?;
        Ok(res)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let wire_count: u32 = match item.len().try_into() {
            Ok(c) => c,
            Err(_) => return Err(ReadError::TooManyItems(item.len(), type_name::<Self::Item>(), "u32"))
        };
        stream.write_item_as::<u32>(&wire_count)?;
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