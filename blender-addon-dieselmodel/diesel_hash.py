import struct

u64_max = 0x10000000000000000

def mix(a, b, c): #-> Tuple[int, int, int]:
    a = (a - b) % u64_max; a = (a - c) % u64_max; a = a ^ ((c >> 43) % u64_max)
    b = (b - c) % u64_max; b = (b - a) % u64_max; b = b ^ ((a >> 9) % u64_max)
    c = (c - a) % u64_max; c = (c - b) % u64_max; c = c ^ ((b >> 8) % u64_max)

    a = (a - b) % u64_max; a = (a - c) % u64_max; a = a ^ ((c >> 38) % u64_max)
    b = (b - c) % u64_max; b = (b - a) % u64_max; b = b ^ ((a >> 23) % u64_max)
    c = (c - a) % u64_max; c = (c - b) % u64_max; c = c ^ ((b >> 5) % u64_max)

    a = (a - b) % u64_max; a = (a - c) % u64_max; a = a ^ ((c >> 35) % u64_max)
    b = (b - c) % u64_max; b = (b - a) % u64_max; b = b ^ ((a >> 49) % u64_max)
    c = (c - a) % u64_max; c = (c - b) % u64_max; c = c ^ ((b >> 11) % u64_max)

    a = (a - b) % u64_max; a = (a - c) % u64_max; a = a ^ ((c >> 12) % u64_max)
    b = (b - c) % u64_max; b = (b - a) % u64_max; b = b ^ ((a >> 18) % u64_max)
    c = (c - a) % u64_max; c = (c - b) % u64_max; c = c ^ ((b >> 22) % u64_max)

    return (a, b, c)

def hash(k: bytes, level: int = 0) -> int:
    lent = len(k)
    a = level
    b = level
    c = 0x9e3779b97f4a7c13

    len_x = 0
    while lent >= 24:
        a = (a + struct.unpack_from("<Q", k, len_x)[0]) % u64_max
        b = (b + struct.unpack_from("<Q", k, len_x+8)[0]) % u64_max
        c = (c + struct.unpack_from("<Q", k, len_x+16)[0]) % u64_max
        (a,b,c) = mix(a,b,c)
        len_x += 24
        lent -= 24
    
    c = (c + len(k)) % u64_max

    if lent <= 23:
        while lent > 0:
            if lent == 23: c = (c + (k[len_x+22] << 56)) % u64_max
            if lent == 22: c = (c + (k[len_x+21] << 48)) % u64_max
            if lent == 21: c = (c + (k[len_x+20] << 40)) % u64_max
            if lent == 20: c = (c + (k[len_x+19] << 32)) % u64_max
            if lent == 19: c = (c + (k[len_x+18] << 24)) % u64_max
            if lent == 18: c = (c + (k[len_x+17] << 16)) % u64_max
            if lent == 17: c = (c + (k[len_x+16] <<  8)) % u64_max

            if lent == 16: b = (b + (k[len_x+15] << 56)) % u64_max
            if lent == 15: b = (b + (k[len_x+14] << 48)) % u64_max
            if lent == 14: b = (b + (k[len_x+13] << 40)) % u64_max
            if lent == 13: b = (b + (k[len_x+12] << 32)) % u64_max
            if lent == 12: b = (b + (k[len_x+11] << 24)) % u64_max
            if lent == 11: b = (b + (k[len_x+10] << 16)) % u64_max
            if lent == 10: b = (b + (k[len_x+ 9] <<  8)) % u64_max
            if lent ==  9: b = (b + (k[len_x+ 8]      )) % u64_max

            if lent ==  8: a = (a + (k[len_x+ 7] << 56)) % u64_max
            if lent ==  7: a = (a + (k[len_x+ 6] << 48)) % u64_max
            if lent ==  6: a = (a + (k[len_x+ 5] << 40)) % u64_max
            if lent ==  5: a = (a + (k[len_x+ 4] << 32)) % u64_max
            if lent ==  4: a = (a + (k[len_x+ 3] << 24)) % u64_max
            if lent ==  3: a = (a + (k[len_x+ 2] << 16)) % u64_max
            if lent ==  2: a = (a + (k[len_x+ 1] <<  8)) % u64_max
            if lent ==  1: a = (a + (k[len_x+ 0]      )) % u64_max
            lent -= 1
    
    mixed = mix(a, b, c)
    return mixed[2]