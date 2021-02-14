use std::rc::Rc;
use std::str;

use fnv::FnvHashMap;

use super::document::*;
use crate::hashindex::{Hash as IdString};
use crate::util::read_helpers::*;
use crate::util::ordered_float::OrderedFloat;
use crate::util::rc_cell::RcCell;

#[derive(Default)]
struct FromBinaryState<'a> {
    input: &'a [u8],
    is_x64: bool,
    is_raid: bool,
    offset_size: usize,
    float_offset: usize,
    string_offset: usize,
    vector_offset: usize,
    quaternion_offset: usize,
    idstring_offset: usize,
    table_offset: usize,
    seen_tables: FnvHashMap<u32, RcCell<InternalTable>>,
    doc: Document
}

impl FromBinaryState<'_> {
    fn by_variant<T>(&self, raid: T, x64: T, x86: T) -> T {
        if self.is_raid { raid } else if self.is_x64 { x64 } else { x86 } 
    }
    fn read_offset(&self, index: usize) -> usize {
        if self.is_x64 {
            read_u64_le(self.input, index) as usize
        }
        else {
            read_u32_le(self.input, index) as usize
        }
    }
    fn read_string(&mut self, index: usize) -> Rc<str> {
        let string_offset_offset = self.string_offset + self.offset_size * (index as usize);
            let string_offset = self.read_offset(string_offset_offset);
            let mut end = string_offset;
            while self.input[end] != 0 {
                end += 1;
            }
            let input_slice_str = str::from_utf8(&self.input[string_offset..end]).unwrap();
            return self.doc.cache_string(input_slice_str);
    }

    fn value_from_binary(&mut self, offset: usize) -> InternalValue {
        let item_type = read_u32_le(self.input, offset);
        let tag = (item_type >> 24) & 0xFF;
        let value = item_type & 0xFFFFFF;
    
        match tag {
            0 => panic!("Nulls in scriptdata aren't supported yet, it's unclear when that would even be useful."),
            1 => InternalValue::Bool(false),
            2 => InternalValue::Bool(true),
            3 => InternalValue::Number(OrderedFloat(read_f32_le(self.input, self.float_offset + (value as usize)*4))),
            4 => InternalValue::String(self.read_string(value as usize)),
            5 => {
                let vector_offset = self.vector_offset + 12 * (value as usize);
                let vec = Vector {
                    x: OrderedFloat(read_f32_le(self.input, vector_offset + 0)),
                    y: OrderedFloat(read_f32_le(self.input, vector_offset + 4)),
                    z: OrderedFloat(read_f32_le(self.input, vector_offset + 8))
                };
                return InternalValue::Vector(vec);
            },
            6 => {
                let quaternion_offset = self.quaternion_offset + 16 * (value as usize);
                let quat = Quaternion {
                    x: OrderedFloat(read_f32_le(self.input, quaternion_offset + 0)),
                    y: OrderedFloat(read_f32_le(self.input, quaternion_offset + 4)),
                    z: OrderedFloat(read_f32_le(self.input, quaternion_offset + 8)),
                    w: OrderedFloat(read_f32_le(self.input, quaternion_offset + 12))
                };
                return InternalValue::Quaternion(quat);
            },
            7 => {
                let idstring_offset = self.idstring_offset + 8 * (value as usize);
                return InternalValue::IdString(IdString(read_u64_le(self.input, idstring_offset)))
            },
            8 => {
                if let Some(tab) = self.seen_tables.get(&value) {
                    return InternalValue::Table(tab.clone());
                }
    
                let table_offset = self.table_offset + (value as usize) * self.by_variant(40, 32, 20);
                let metatable_index = self.read_offset(table_offset);
                let metatable_str = if metatable_index > 0 { Some(self.read_string(metatable_index)) } else { None };
                let item_count = if self.is_raid {
                    read_u64_le(self.input, table_offset + self.offset_size) as usize
                }
                else {
                    read_u32_le(self.input, table_offset + self.offset_size) as usize
                };
                let items_offset = self.read_offset(table_offset + self.offset_size + self.by_variant(16, 8, 8));
    
                let mut table = InternalTable::new();
                table.set_metatable(metatable_str);
                for i in 0..item_count {
                    let item_offset = items_offset + i * 8;
                    let key = self.value_from_binary(item_offset);
                    let value = self.value_from_binary(item_offset+4);
                    table.insert(key, value);
                }
                
                let tab_ref = RcCell::new(table);

                self.seen_tables.insert(value, tab_ref.clone());
                return InternalValue::Table(tab_ref);
            },
            _ => panic!("Unrecognised tag {}", tag)
        }
    }
}

pub fn from_binary(input: &[u8], is_raid: bool ) -> Document {
    let is_x64 = is_raid || read_u32_le(input, 0) == 568494624;
    
    let mut state = FromBinaryState {
        input,
        is_raid,
        is_x64,
        offset_size: if is_x64 { 8 } else { 4 },
        .. FromBinaryState::default()
    };
    
    let header_pad = state.by_variant(24, 16, 12);
    state.float_offset      = state.read_offset(header_pad + (header_pad + state.offset_size) * 0);
    state.string_offset     = state.read_offset(header_pad + (header_pad + state.offset_size) * 1);
    state.vector_offset     = state.read_offset(header_pad + (header_pad + state.offset_size) * 2);
    state.quaternion_offset = state.read_offset(header_pad + (header_pad + state.offset_size) * 3);
    state.idstring_offset   = state.read_offset(header_pad + (header_pad + state.offset_size) * 4);
    state.table_offset      = state.read_offset(header_pad + (header_pad + state.offset_size) * 5);

    let root_offset = state.by_variant(200, 152, 100);
    let root = state.value_from_binary(root_offset);
    state.doc.set_root(Some(root));
    
    return state.doc;
}