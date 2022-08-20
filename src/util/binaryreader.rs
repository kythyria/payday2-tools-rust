use std::io::{Read, Write, Error as IoError};

/// Represents finding an enum discriminant that isn't recognised.
/// 
/// The macro-generated readers assume they can `?` this into `ItemReader::Error`.
pub struct BadDiscriminant<T>(T);

/// Defines how to read/write a `T` from/to a stream. TODO: bytemuck integration.
pub trait ItemReader {
    type Error;
    type Item;

    fn read_from_stream<R: Read>(stream: &mut R) -> Result<Self::Item, Self::Error>;
    fn write_to_stream<W: Write>(stream: &mut W, item: &Self::Item) -> Result<(), IoError>;
}

/// Extend a `Read` to be able to read objects, not just bytes.
pub trait ReadExt: Read {
    fn read_item<I: ItemReader<Item=I>>(&mut self) -> Result<I, I::Error>;
    fn read_item_as<P: ItemReader>(&mut self) -> Result<P::Item, P::Error>;
}

pub trait WriteExt: Write {
    fn write_item<I: ItemReader>(&mut self, item: &I) -> Result<(), I::Error>;
    fn write_item_as<P: ItemReader>(&mut self, item: &P::Item) -> Result<(), P::Error>;
}

// https://discord.com/channels/273534239310479360/1009669096704573511