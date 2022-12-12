use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::rc::Rc;

use thiserror::Error;

use crate::util::binaryreader::{ReadExt, WriteExt, ReadError};
use super::Section as Section;

pub struct DieselContainer {
    sections: HashMap<u32, Box<Section>>,
    section_order: Vec<u32>,
    next_id: u32
}

impl Default for DieselContainer {
    fn default() -> Self {
        Self {
            sections: Default::default(),
            section_order: Default::default(),
            next_id: 1000
        }
    }
}

impl DieselContainer {
    pub fn new() -> Self { Default::default() }
    pub fn with_capacity(capacity: usize) -> Self {
        DieselContainer {
            sections: HashMap::with_capacity(capacity),
            section_order: Vec::with_capacity(capacity),
            next_id: 1000
        }
    }
    
    pub fn get(&self, id: u32) -> Option<&Section> { self.sections.get(&id).map(|i| i.as_ref()) }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Section> { self.sections.get_mut(&id).map(|i| i.as_mut()) }


    pub fn insert(&mut self, id: u32, sec: impl Into<Section>) -> Option<Box<Section>> {
        let s = self.sections.insert(id, Box::new(sec.into()));
        if s.is_some()  {
            return s;
        }
        else {
            self.section_order.push(id);
            return None;
        }
    }
    pub fn push(&mut self, sec: impl Into<Section>) -> u32 {
        while self.sections.contains_key(&self.next_id) {
            self.next_id += 1;
        }
        self.sections.insert(self.next_id, Box::new(sec.into()));
        self.section_order.push(self.next_id);
        self.next_id
    }
    pub fn remove(&mut self, sid: u32) -> Option<Box<Section>> {
        let sec = self.sections.remove(&sid)?;
        let pos = self.section_order.iter().enumerate().find(|(_, id)| sid == **id).unwrap().0;
        self.section_order.remove(pos);
        Some(sec)
    }
    
    pub fn sections(&self) -> impl Iterator<Item=(u32, &Section)> {
        self.section_order.iter().map(move |id| {
            (*id, self.sections[id].as_ref())
        })
    }
}

impl crate::util::binaryreader::ItemReader for DieselContainer {
    type Error = ReadError;
    type Item = Self;

    fn read_from_stream<R: ReadExt>(stream: &mut R) -> Result<Self::Item, Self::Error> {
        let (section_count, _) = read_fdm_header(stream)?;

        let mut out = DieselContainer::with_capacity(section_count as usize);

        for _ in 0..section_count {
            let sec_type: super::SectionType = stream.read_item()?;
            let sec_id: u32 = stream.read_item()?;
            let sec_data: Vec<u8> = stream.read_item()?;
            
            let sec = super::read_section(sec_type, &sec_data)?;
            let prev = out.insert(sec_id, sec);
            if prev.is_some() {
                return Err(ReadError::Schema("Multiple sections with the same ID"));
            }
        }

        Ok(out)
    }

    fn write_to_stream<W: WriteExt>(stream: &mut W, item: &Self::Item) -> Result<(), Self::Error> {
        let sections: Vec<_> = item.sections().map(|(id,sec)| {
            let mut v = Vec::new();
            sec.write_data(&mut v).unwrap();
            (sec.tag(), id, v)
        }).collect();
        let total_len: usize = sections.iter().map(|i| i.2.len()).sum::<usize>() + 4 + 4;
        stream.write_item_as::<u32>(&sections.len().try_into().unwrap())?;
        stream.write_item_as::<u32>(&total_len.try_into().unwrap())?;
        for s in sections {
            stream.write_item(&s)?;
        }
        Ok(())
    }
}

fn read_fdm_header(stream: &mut impl ReadExt) -> Result<(u32, u32), ReadError> {
    let sig: u32 = stream.read_item()?;
    let length: u32 = stream.read_item()?;
    let section_count = if sig != 0xFFFFFFFF { sig } else { stream.read_item()? };
    Ok((section_count, length))
}