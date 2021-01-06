use std::convert::TryFrom;
use std::num::Wrapping;

fn mix64(a : &mut Wrapping<u64>, b: &mut Wrapping<u64>, c: &mut Wrapping<u64>) {
    *a -= *b; *a -= *c; *a ^= *c >> 43;
    *b -= *c; *b -= *a; *b ^= *a << 9;
    *c -= *a; *c -= *b; *c ^= *b >> 8;
    *a -= *b; *a -= *c; *a ^= *c >> 38;
    *b -= *c; *b -= *a; *b ^= *a << 23;
    *c -= *a; *c -= *b; *c ^= *b >> 5;
    *a -= *b; *a -= *c; *a ^= *c >> 35;
    *b -= *c; *b -= *a; *b ^= *a << 49;
    *c -= *a; *c -= *b; *c ^= *b >> 11;
    *a -= *b; *a -= *c; *a ^= *c >> 12;
    *b -= *c; *b -= *a; *b ^= *a << 18;
    *c -= *a; *c -= *b; *c ^= *b >> 22;
}

fn read_le_u64(k: &[u8], i: usize) -> Wrapping<u64> {
    let val = u64::from(k[i + 0])
        + (u64::from(k[i + 1]) << 8) 
        + (u64::from(k[i + 2]) << 16) 
        + (u64::from(k[i + 3]) << 24)
        + (u64::from(k[i + 4]) << 32)
        + (u64::from(k[i + 5]) << 40)
        + (u64::from(k[i + 6]) << 48)
        + (u64::from(k[i + 7]) << 56);
    //let val = u64::from_le_bytes(k[i..(i+8)].try_into().unwrap());
    return Wrapping(val);
}

fn wu64_from(v: u8) -> Wrapping<u64> {
    return Wrapping(u64::from(v));
}

//pub fn hash(k: &[u8]) -> u64 { return hash_level(k, 0); }
pub fn hash_str(s: &str) -> u64 { return hash_level(s.as_bytes(), 0); }

pub fn hash_level(k : &[u8], level: u64) -> u64 {
    let mut len = k.len();
    let mut a = Wrapping(level);
    let mut b = Wrapping(level);
    let mut c = Wrapping::<u64>(0x9e3779b97f4a7c13);
    
    let mut len_x = 0;
    while len >= 24 {
        a += read_le_u64(k, len_x);
        b += read_le_u64(k, len_x+8);
        c += read_le_u64(k, len_x+16);
        mix64(&mut a, &mut b, &mut c);
        len_x += 24; len -= 24;
    }

    c += Wrapping(u64::try_from(k.len()).expect("What, are you running this on a machine with 128-bit memory addresses? o.O"));

    if len <= 23 {
        while len > 0 {
            c += match len {
                23 => wu64_from(k[len_x + 22]) << 56,
                22 => wu64_from(k[len_x + 21]) << 48,
                21 => wu64_from(k[len_x + 20]) << 40,
                20 => wu64_from(k[len_x + 19]) << 32,
                19 => wu64_from(k[len_x + 18]) << 24,
                18 => wu64_from(k[len_x + 17]) << 16,
                17 => wu64_from(k[len_x + 16]) << 8,
                _ => Wrapping(0)
            };
            b += match len {
                16 => wu64_from(k[len_x + 15]) << 56,
                15 => wu64_from(k[len_x + 14]) << 48,
                14 => wu64_from(k[len_x + 13]) << 40,
                13 => wu64_from(k[len_x + 12]) << 32,
                12 => wu64_from(k[len_x + 11]) << 24,
                11 => wu64_from(k[len_x + 10]) << 16,
                10 => wu64_from(k[len_x + 9]) << 8,
                9  => wu64_from(k[len_x + 8]),
                _ => Wrapping(0)
            };
            a += match len {
                8 => wu64_from(k[len_x + 7]) << 56,
                7 => wu64_from(k[len_x + 6]) << 48,
                6 => wu64_from(k[len_x + 5]) << 40,
                5 => wu64_from(k[len_x + 4]) << 32,
                4 => wu64_from(k[len_x + 3]) << 24,
                3 => wu64_from(k[len_x + 2]) << 16,
                2 => wu64_from(k[len_x + 1]) << 8,
                1 => wu64_from(k[len_x + 0]),
                _ => Wrapping(0)
            };
            len -= 1;
        }
    }
    mix64(&mut a, &mut b, &mut c);
    return c.0;
}