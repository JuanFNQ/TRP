//! Tests of the X-Ray archives' checksum and their LZO and LZHUF unpacking.

use super::*;

#[test]
fn the_checksum_is_the_usual_crc_32() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

/// LZO1X by hand: three literals as the stream's first byte says, a
/// match of nine bytes three back, the end.
#[test]
fn lzo1x_literals_a_match_and_the_end() {
    let packed = [20, b'a', b'b', b'c', 32 + 7, 2 << 2, 0, 0x11, 0, 0];
    assert_eq!(unpack_lzo1x(&packed, 12).unwrap(), b"abcabcabcabc");
}

/// The short matches and the literals a match's last two bits bring.
#[test]
fn lzo1x_short_matches_and_trailing_literals() {
    // Four literals ("abcd"); a match of three bytes two back
    // (code 64..: length (t >> 5) + 1 = 3, distance 1 + ((t >> 2) & 7)
    // + (next << 3) = 2), with one literal after it ("x"); then a
    // match of two bytes one back (a code under 16 after trailing
    // literals: distance 1 + (t >> 2) + (next << 2) = 1); the end.
    let packed = [21, b'a', b'b', b'c', b'd', (2 << 5) | (1 << 2) | 1, 0, b'x', 0, 0, 0x11, 0, 0];
    assert_eq!(unpack_lzo1x(&packed, 10).unwrap(), b"abcdcdcxxx");
}

/// An encoder of LZHUF's codes for a list of literals and matches,
/// written from the same description, to unpack again.
fn pack_lzhuf(tokens: &[(u8, usize, usize)], size: usize) -> Vec<u8> {
    // Bits from the top of each byte down.
    let mut bits: Vec<bool> = Vec::new();
    let put = |bits: &mut Vec<bool>, length: u32, value: u32| {
        for k in (0..length).rev() {
            bits.push((value >> k) & 1 == 1);
        }
    };
    // A code is the path from the root to its leaf: the second child
    // of a pair is a one.
    let code = |tree: &mut Tree, bits: &mut Vec<bool>, code: usize| {
        let mut path = Vec::new();
        let mut k = tree.parent[code + NODES];
        while k != ROOT {
            path.push(k & 1 == 1);
            k = tree.parent[k];
        }
        bits.extend(path.iter().rev());
        tree.update(code);
    };
    let mut position_code = [0u32; 64];
    let mut first = 0u32;
    for (c, length) in POSITION_LENGTH.iter().enumerate() {
        position_code[c] = first >> (8 - length);
        first += 1 << (8 - length);
    }
    let mut tree = Tree::new();
    for (literal, length, distance) in tokens {
        if *length == 0 {
            code(&mut tree, &mut bits, *literal as usize);
        } else {
            code(&mut tree, &mut bits, 255 - SHORTEST + length);
            let d = (distance - 1) as u32;
            let high = (d >> 6) as usize;
            put(&mut bits, POSITION_LENGTH[high] as u32, position_code[high]);
            put(&mut bits, 6, d & 0x3F);
        }
    }
    let mut packed = (size as u32).to_le_bytes().to_vec();
    packed.extend(bits.chunks(8).map(|byte| byte.iter().enumerate().fold(0u8, |sum, (k, bit)| sum | ((*bit as u8) << (7 - k)))));
    packed
}

/// Literals and matches of every length and at every reach, enough of
/// them that the tree is rebuilt several times.
#[test]
fn lzhuf_unpacks_what_its_codes_say() {
    let mut tokens = Vec::new();
    let mut expected: Vec<u8> = Vec::new();
    let mut seed = 12345u32;
    let mut random = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) as usize
    };
    while expected.len() < 200_000 {
        if expected.len() < 8 || random() % 3 == 0 {
            let byte = (random() % 40) as u8 + b'a';
            tokens.push((byte, 0, 0));
            expected.push(byte);
        } else {
            let length = 3 + random() % (LONGEST - 2);
            let distance = 1 + random() % expected.len().min(WINDOW - LONGEST - 1);
            tokens.push((0, length, distance));
            let from = expected.len() - distance;
            for k in 0..length {
                expected.push(expected[from + k]);
            }
        }
    }
    let packed = pack_lzhuf(&tokens, expected.len());
    assert_eq!(unpack_lzhuf(&packed).unwrap(), expected);
}
