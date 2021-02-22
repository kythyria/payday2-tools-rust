use fnv::{FnvHashMap as HashMap};

pub struct DieselFont {
    kernings: Vec<Kerning>,
    texture_width: i32,
    texture_height: i32,
    name: String,
    info_size: i64,
    common_base: i32,
    line_height: i32,
    unknown_1: i64,
    unknown_2: i64,
    unknown_3: i64,
    unknown_4: i32,
    unknown_5: i64,
    unknown_7: i32,
    characters: BTreeMap<char, Character>,
}

pub struct Kerning {
    char_1: char,
    char_2: char,
    unknown_1: u8,
    unknown_2: u8,
    unknown_3: u8
    unknown_4: u8
}

pub struct Character {
    id: i32,
    character: char,
    x: i16,
    y: i16,
    w: u8,
    h: u8,
    x_advance: u8,
    x_offset: i8,
    y_offset: i16,
}

