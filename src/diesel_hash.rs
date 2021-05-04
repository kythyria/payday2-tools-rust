const fn const_mix64(mut a: u64, mut b: u64, mut c: u64) -> (u64, u64, u64) {
    a = a.wrapping_sub(b); a = a.wrapping_sub(c); a ^= c.wrapping_shr(43);
    b = b.wrapping_sub(c); b = b.wrapping_sub(a); b ^= a.wrapping_shl(9);
    c = c.wrapping_sub(a); c = c.wrapping_sub(b); c ^= b.wrapping_shr(8);
    a = a.wrapping_sub(b); a = a.wrapping_sub(c); a ^= c.wrapping_shr(38);
    b = b.wrapping_sub(c); b = b.wrapping_sub(a); b ^= a.wrapping_shl(23);
    c = c.wrapping_sub(a); c = c.wrapping_sub(b); c ^= b.wrapping_shr(5);
    a = a.wrapping_sub(b); a = a.wrapping_sub(c); a ^= c.wrapping_shr(35);
    b = b.wrapping_sub(c); b = b.wrapping_sub(a); b ^= a.wrapping_shl(49);
    c = c.wrapping_sub(a); c = c.wrapping_sub(b); c ^= b.wrapping_shr(11);
    a = a.wrapping_sub(b); a = a.wrapping_sub(c); a ^= c.wrapping_shr(12);
    b = b.wrapping_sub(c); b = b.wrapping_sub(a); b ^= a.wrapping_shl(18);
    c = c.wrapping_sub(a); c = c.wrapping_sub(b); c ^= b.wrapping_shr(22);

    return (a,b,c);
}

const fn read_le_u64(k: &[u8], i: usize) -> u64 {
    let val = (k[i + 0] as u64)
        + ((k[i + 1] as u64) << 8) 
        + ((k[i + 2] as u64) << 16) 
        + ((k[i + 3] as u64) << 24)
        + ((k[i + 4] as u64) << 32)
        + ((k[i + 5] as u64) << 40)
        + ((k[i + 6] as u64) << 48)
        + ((k[i + 7] as u64) << 56);
    return val
}

//pub fn hash(k: &[u8]) -> u64 { return hash_level(k, 0); }
pub const fn hash_str(s: &str) -> u64 { return hash_level(s.as_bytes(), 0); }

/// Like hash_str, but if the string is 16 hex digits assume it's a hash printed through {:x}
///
/// Basically it's the opposite of HashStr's Display impl.
pub fn from_str(s: &str) -> u64 {
    if s.len() == 16 {
        if let Ok(i) = u64::from_str_radix(s, 16) {
            return i;
        }
    }
    hash_str(s)
}

/// Try to parse with the specific radix and also recognise the `@ID...@` syntax.
pub fn parse_flexibly(s: &str, radix: u32) -> Result<u64, ()> {
    if s.len() == 20 {
        if &s[0..3] == "@ID" && &s[19..20] == "@" {
            let s = &s[3..19];
            if let Ok(i) = u64::from_str_radix(s, 16) {
                return Ok(u64::from_be_bytes(i.to_le_bytes()));
            }
        }
    }
    if let Ok(i) = u64::from_str_radix(s, radix) {
        return Ok(i);
    }
    return Err(());
}

pub const fn hash_level(k : &[u8], level: u64) -> u64 {
    let mut len: u64 = k.len() as u64;
    let mut a: u64 = level;
    let mut b: u64 = level;
    let mut c: u64 = 0x9e3779b97f4a7c13;
    
    let mut len_x = 0;
    while len >= 24 {
        a = a.wrapping_add(read_le_u64(k, len_x));
        b = b.wrapping_add(read_le_u64(k, len_x+8));
        c = c.wrapping_add(read_le_u64(k, len_x+16));
        let mixed = const_mix64(a, b, c);
        a = mixed.0;
        b = mixed.1;
        c = mixed.2;
        len_x += 24; len -= 24;
    }

    c = c.wrapping_add(k.len() as u64);

    if len <= 23 {
        while len > 0 {
            c = c.wrapping_add(match len {
                23 => (k[len_x + 22] as u64) << 56,
                22 => (k[len_x + 21] as u64) << 48,
                21 => (k[len_x + 20] as u64) << 40,
                20 => (k[len_x + 19] as u64) << 32,
                19 => (k[len_x + 18] as u64) << 24,
                18 => (k[len_x + 17] as u64) << 16,
                17 => (k[len_x + 16] as u64) << 8,
                _ => 0
            });
            b = b.wrapping_add(match len {
                16 => (k[len_x + 15] as u64) << 56,
                15 => (k[len_x + 14] as u64) << 48,
                14 => (k[len_x + 13] as u64) << 40,
                13 => (k[len_x + 12] as u64) << 32,
                12 => (k[len_x + 11] as u64) << 24,
                11 => (k[len_x + 10] as u64) << 16,
                10 => (k[len_x + 9] as u64) << 8,
                9  => (k[len_x + 8] as u64),
                _ => 0
            });
            a = a.wrapping_add(match len {
                8 => (k[len_x + 7] as u64) << 56,
                7 => (k[len_x + 6] as u64) << 48,
                6 => (k[len_x + 5] as u64) << 40,
                5 => (k[len_x + 4] as u64) << 32,
                4 => (k[len_x + 3] as u64) << 24,
                3 => (k[len_x + 2] as u64) << 16,
                2 => (k[len_x + 1] as u64) << 8,
                1 => (k[len_x + 0] as u64),
                _ => 0
            });
            len -= 1;
        }
    }
    let mixed = const_mix64(a, b, c);
    c = mixed.2;
    return c;
}

pub const EMPTY: u64 = hash_str("");