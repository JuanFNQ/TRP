//! The X-Ray engine's packed archives (`.db0`, `.db1`, ...): where the
//! other game keeps the models and pictures that are not loose
//! (`docs/findings.md`, "the other game's packed archives"). Read in place;
//! nothing is unpacked to disk.
//!
//! The format is the engine's own, read from its published source
//! (OpenXRay, `xrCore/LocatorAPI.cpp`, `ProcessArchive` and the reading of
//! a file from an archive): an archive is a file of chunks, each a number,
//! a length and a body. Chunk 666 is a short text (the folder its names
//! are under); chunk 0 holds the files' bytes; chunk 1 is the table of the
//! files, packed where its number has the top bit set. Each entry of the
//! table: a length, then the file's size, its packed size, a checksum, its
//! name, and where its bytes begin in the archive.
//!
//! Two packings, both published algorithms: the table's is Okumura and
//! Yoshizaki's LZHUF (1988; `xrCore/LzHuf.cpp`, `_decompressLZ`), its
//! unpacked length first; a file whose packed size is not its size is
//! packed by LZO1X (Oberhumer; `xrCore/rt_compressor.cpp`,
//! `rtc_decompress`). The checksum is the usual CRC-32 of the file's bytes,
//! and is checked.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, ensure, Context, Result};

/// The chunk of the table, and the mark of a packed chunk.
const TABLE: u32 = 1;
const PACKED: u32 = 0x8000_0000;

/// Where a file is in its archive.
#[derive(Debug, Clone, Copy)]
struct Entry {
    offset: u32,
    size: u32,
    packed: u32,
    crc: u32,
}

/// An archive's table: every file by its name under `gamedata`, in lower
/// case with `\` between its folders.
pub struct Archive {
    pub path: PathBuf,
    files: HashMap<String, Entry>,
}

impl std::fmt::Debug for Archive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Archive({}, {} files)", self.path.display(), self.files.len())
    }
}

/// A name as an archive's table has it.
fn key(name: &str) -> String {
    name.trim_start_matches(['\\', '/']).replace('/', "\\").to_lowercase()
}

impl Archive {
    /// Reads an archive's table: its chunks are walked by their headers,
    /// and only the table is read.
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).with_context(|| path.display().to_string())?;
        let length = file.metadata()?.len();
        let mut at = 0u64;
        let mut table = None;
        while at + 8 <= length {
            let mut header = [0u8; 8];
            file.seek(SeekFrom::Start(at))?;
            file.read_exact(&mut header)?;
            let (id, size) = (u32::from_le_bytes(header[..4].try_into()?), u32::from_le_bytes(header[4..].try_into()?));
            if id & !PACKED == TABLE {
                let mut body = vec![0u8; size as usize];
                file.read_exact(&mut body)?;
                table = Some(if id & PACKED != 0 { unpack_lzhuf(&body)? } else { body });
                break;
            }
            at += 8 + size as u64;
        }
        let table = table.ok_or_else(|| anyhow!("{}: no table of files", path.display()))?;
        let mut files = HashMap::new();
        let mut at = 0;
        while at < table.len() {
            ensure!(at + 2 <= table.len(), "the table ends in an entry");
            let length = u16::from_le_bytes([table[at], table[at + 1]]) as usize;
            at += 2;
            ensure!(length >= 16 && at + length <= table.len(), "an entry of the table of {length} bytes");
            let entry = &table[at..at + length];
            at += length;
            let word = |from: usize| u32::from_le_bytes([entry[from], entry[from + 1], entry[from + 2], entry[from + 3]]);
            let name = String::from_utf8_lossy(&entry[12..length - 4]);
            // A folder is listed too, with nothing in it.
            if name.ends_with('\\') {
                continue;
            }
            files.insert(key(&name), Entry { size: word(0), packed: word(4), crc: word(8), offset: word(length - 4) });
        }
        Ok(Self { path: path.to_owned(), files })
    }

    /// How many files it has.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Every name it has that has `part` in it.
    pub fn names_with(&self, part: &str) -> Vec<&str> {
        let part = key(part);
        let mut found: Vec<&str> = self.files.keys().filter(|name| name.contains(&part)).map(String::as_str).collect();
        found.sort();
        found
    }

    /// Whether it has a file of that name.
    pub fn has(&self, name: &str) -> bool {
        self.files.contains_key(&key(name))
    }

    /// A file's bytes, unpacked and checked; none where it has no file of
    /// that name.
    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(entry) = self.files.get(&key(name)).copied() else { return Ok(None) };
        let mut file = File::open(&self.path).with_context(|| self.path.display().to_string())?;
        file.seek(SeekFrom::Start(entry.offset as u64))?;
        let mut stored = vec![0u8; entry.packed as usize];
        file.read_exact(&mut stored).with_context(|| format!("{name} in {}", self.path.display()))?;
        let bytes = if entry.packed == entry.size { stored } else { unpack_lzo1x(&stored, entry.size as usize).with_context(|| format!("{name} in {}", self.path.display()))? };
        ensure!(bytes.len() == entry.size as usize, "{name} in {}: {} bytes unpacked where {} are listed", self.path.display(), bytes.len(), entry.size);
        ensure!(crc32(&bytes) == entry.crc, "{name} in {}: its checksum is not the one listed", self.path.display());
        Ok(Some(bytes))
    }
}

/// Every archive of the other game's folder, the one that wins first:
/// those under `db\patches` and `db\addons` (which the game mounts to lie
/// over the rest), then all others, a later name before an earlier.
/// (`PROVISIONAL`: which of two archives with the same file wins is not
/// read from the engine; none of the files read here is in two.)
pub fn archives_of(game: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.len() >= 3 && e[..2].eq_ignore_ascii_case("db") && e[2..].chars().all(|c| c.is_ascii_digit())) {
                out.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(&game.join("db"), &mut found);
    let over = |path: &PathBuf| path.components().any(|part| part.as_os_str().eq_ignore_ascii_case("patches") || part.as_os_str().eq_ignore_ascii_case("addons"));
    found.sort_by(|a, b| over(b).cmp(&over(a)).then_with(|| b.cmp(a)));
    found
}

/// The usual CRC-32 (the reflected polynomial 0xEDB88320, begun and ended
/// with every bit set), as the engine's `crc32`.
pub fn crc32(bytes: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        std::array::from_fn(|n| {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            c
        })
    });
    !bytes.iter().fold(!0u32, |crc, byte| table[((crc ^ *byte as u32) & 0xFF) as usize] ^ (crc >> 8))
}

// ------------------------------------------------------------------ LZHUF

/// The window, the longest match, the shortest one worth a code.
const WINDOW: usize = 4096;
const LONGEST: usize = 60;
const SHORTEST: usize = 2;
/// The codes: 256 bytes and the lengths of a match; the tree's nodes; its
/// root; the count at which the tree is rebuilt with every count halved.
const CODES: usize = 256 - SHORTEST + LONGEST;
const NODES: usize = CODES * 2 - 1;
const ROOT: usize = NODES - 1;
/// The engine's is half the 1988 program's (0x8000): read from the
/// archives themselves on 2026-10-14, every table of the 68 decoding whole
/// at this and the twelve longer ones going wrong at the other, where the
/// tree is first rebuilt.
const MOST: u32 = 0x4000;

/// The length of the code of a match's distance's upper six bits, for
/// each of their 64 values: the codes themselves are given out in order,
/// each the one before and one more at its own length.
const POSITION_LENGTH: [u8; 64] = {
    let mut lengths = [0u8; 64];
    let mut c = 0;
    while c < 64 {
        lengths[c] = match c {
            0 => 3,
            1..=3 => 4,
            4..=11 => 5,
            12..=23 => 6,
            24..=47 => 7,
            _ => 8,
        };
        c += 1;
    }
    lengths
};

/// The adaptive tree both ends keep: each node's count, its first child
/// (a leaf's is the code and `NODES`), and its parent (leaves' from
/// `NODES` on).
struct Tree {
    count: Vec<u32>,
    child: Vec<usize>,
    parent: Vec<usize>,
}

impl Tree {
    fn new() -> Self {
        let mut tree = Self { count: vec![0; NODES + 1], child: vec![0; NODES], parent: vec![0; NODES + CODES] };
        for code in 0..CODES {
            tree.count[code] = 1;
            tree.child[code] = code + NODES;
            tree.parent[code + NODES] = code;
        }
        let (mut i, mut j) = (0, CODES);
        while j <= ROOT {
            tree.count[j] = tree.count[i] + tree.count[i + 1];
            tree.child[j] = i;
            tree.parent[i] = j;
            tree.parent[i + 1] = j;
            i += 2;
            j += 1;
        }
        tree.count[NODES] = 0xFFFF;
        tree.parent[ROOT] = 0;
        tree
    }

    /// Rebuilt with every leaf's count halved (rounded up), when the root's
    /// reaches `MOST`.
    fn rebuild(&mut self) {
        let mut j = 0;
        for i in 0..NODES {
            if self.child[i] >= NODES {
                self.count[j] = self.count[i].div_ceil(2);
                self.child[j] = self.child[i];
                j += 1;
            }
        }
        let mut i = 0;
        for j in CODES..NODES {
            let sum = self.count[i] + self.count[i + 1];
            let mut k = j;
            while k > 0 && sum < self.count[k - 1] {
                k -= 1;
            }
            self.count.copy_within(k..j, k + 1);
            self.count[k] = sum;
            self.child.copy_within(k..j, k + 1);
            self.child[k] = i;
            i += 2;
        }
        for i in 0..NODES {
            let k = self.child[i];
            self.parent[k] = i;
            if k < NODES {
                self.parent[k + 1] = i;
            }
        }
    }

    /// A code's count and those above it one more, nodes swapped to keep
    /// the counts in order.
    fn update(&mut self, code: usize) {
        if self.count[ROOT] == MOST {
            self.rebuild();
        }
        let mut c = self.parent[code + NODES];
        loop {
            self.count[c] += 1;
            let k = self.count[c];
            if k > self.count[c + 1] {
                let mut l = c + 1;
                while k > self.count[l + 1] {
                    l += 1;
                }
                self.count[c] = self.count[l];
                self.count[l] = k;
                let i = self.child[c];
                self.parent[i] = l;
                if i < NODES {
                    self.parent[i + 1] = l;
                }
                let j = self.child[l];
                self.child[l] = i;
                self.parent[j] = c;
                if j < NODES {
                    self.parent[j + 1] = c;
                }
                self.child[c] = j;
                c = l;
            }
            c = self.parent[c];
            if c == 0 {
                break;
            }
        }
    }
}

/// Bits read from the top of each byte down; past the end, nothing.
struct Bits<'a> {
    bytes: &'a [u8],
    at: usize,
    held: u32,
    count: u32,
}

impl Bits<'_> {
    fn fill(&mut self) {
        while self.count <= 8 {
            let byte = self.bytes.get(self.at).copied().unwrap_or(0) as u32;
            self.at += 1;
            self.held |= byte << (8 - self.count);
            self.count += 8;
        }
    }

    fn bit(&mut self) -> usize {
        self.fill();
        let bit = (self.held >> 15) & 1;
        self.held = (self.held << 1) & 0xFFFF;
        self.count -= 1;
        bit as usize
    }

    fn byte(&mut self) -> usize {
        self.fill();
        let byte = (self.held >> 8) & 0xFF;
        self.held = (self.held << 8) & 0xFFFF;
        self.count -= 8;
        byte as usize
    }
}

/// Unpacks LZHUF: the unpacked length (four bytes), then the codes.
pub fn unpack_lzhuf(packed: &[u8]) -> Result<Vec<u8>> {
    ensure!(packed.len() >= 4, "too short to be packed");
    let size = u32::from_le_bytes(packed[..4].try_into()?) as usize;
    // (A guard against a length that is not one before it is made room for.)
    ensure!(size <= packed.len().saturating_mul(256).max(1 << 20), "{size} bytes unpacked from {}", packed.len());
    let mut out = Vec::with_capacity(size);
    let mut bits = Bits { bytes: &packed[4..], at: 0, held: 0, count: 0 };
    let mut tree = Tree::new();
    let mut window = [b' '; WINDOW];
    let mut r = WINDOW - LONGEST;
    // The first byte of a position's code says its upper six bits.
    let mut upper = [(0u8, 0u8); 256];
    let mut first = 0usize;
    for (c, length) in POSITION_LENGTH.iter().enumerate() {
        let span = 1 << (8 - length);
        for slot in &mut upper[first..first + span] {
            *slot = (c as u8, *length);
        }
        first += span;
    }
    while out.len() < size {
        let mut c = tree.child[ROOT];
        while c < NODES {
            c = tree.child[c + bits.bit()];
        }
        let code = c - NODES;
        tree.update(code);
        if code < 256 {
            out.push(code as u8);
            window[r] = code as u8;
            r = (r + 1) & (WINDOW - 1);
            continue;
        }
        let mut i = bits.byte();
        let (high, length) = upper[i];
        for _ in 0..length - 2 {
            i = (i << 1) + bits.bit();
        }
        let distance = ((high as usize) << 6) | (i & 0x3F);
        let from = (r + WINDOW - distance - 1) & (WINDOW - 1);
        for k in 0..code - 255 + SHORTEST {
            let byte = window[(from + k) & (WINDOW - 1)];
            out.push(byte);
            window[r] = byte;
            r = (r + 1) & (WINDOW - 1);
        }
    }
    out.truncate(size);
    Ok(out)
}

// ------------------------------------------------------------------ LZO1X

/// Unpacks LZO1X to `size` bytes.
pub fn unpack_lzo1x(packed: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut ip = 0usize;
    let next = |ip: &mut usize| -> Result<usize> {
        let byte = *packed.get(*ip).ok_or_else(|| anyhow!("the packed bytes end early"))?;
        *ip += 1;
        Ok(byte as usize)
    };
    // A count past a code's own bits: every zero byte 255 more, then the
    // first that is not.
    let long = |ip: &mut usize| -> Result<usize> {
        let mut count = 0;
        while *packed.get(*ip).ok_or_else(|| anyhow!("the packed bytes end early"))? == 0 {
            count += 255;
            *ip += 1;
        }
        Ok(count + next(ip)?)
    };
    let literals = |out: &mut Vec<u8>, ip: &mut usize, count: usize| -> Result<()> {
        let run = packed.get(*ip..*ip + count).ok_or_else(|| anyhow!("a run of {count} bytes goes past the packed bytes"))?;
        out.extend_from_slice(run);
        *ip += count;
        Ok(())
    };
    let copy = |out: &mut Vec<u8>, distance: usize, count: usize| -> Result<()> {
        ensure!(distance >= 1 && distance <= out.len(), "a match {distance} bytes back, of {} made", out.len());
        let from = out.len() - distance;
        for k in 0..count {
            out.push(out[from + k]);
        }
        Ok(())
    };
    enum Step {
        Literals,
        AfterLiterals,
        Match(usize),
        Trailing(usize),
    }
    let mut step = Step::Literals;
    if packed.first().is_some_and(|first| *first > 17) {
        let t = next(&mut ip)? - 17;
        if t < 4 {
            step = Step::Trailing(t);
        } else {
            literals(&mut out, &mut ip, t)?;
            step = Step::AfterLiterals;
        }
    }
    loop {
        ensure!(out.len() <= size, "more than the {size} bytes listed");
        step = match step {
            Step::Literals => {
                let mut t = next(&mut ip)?;
                if t >= 16 {
                    Step::Match(t)
                } else {
                    if t == 0 {
                        t = 15 + long(&mut ip)?;
                    }
                    literals(&mut out, &mut ip, t + 3)?;
                    Step::AfterLiterals
                }
            }
            // Just after a run of literals, a code under 16 is a match of
            // three bytes from beyond the short reach.
            Step::AfterLiterals => {
                let t = next(&mut ip)?;
                if t >= 16 {
                    Step::Match(t)
                } else {
                    let distance = 1 + 0x0800 + (t >> 2) + (next(&mut ip)? << 2);
                    copy(&mut out, distance, 3)?;
                    Step::Trailing(packed[ip - 2] as usize & 3)
                }
            }
            Step::Match(t) => {
                if t >= 64 {
                    let distance = 1 + ((t >> 2) & 7) + (next(&mut ip)? << 3);
                    copy(&mut out, distance, (t >> 5) + 1)?;
                } else if t >= 32 {
                    let mut count = t & 31;
                    if count == 0 {
                        count = 31 + long(&mut ip)?;
                    }
                    let (low, high) = (next(&mut ip)?, next(&mut ip)?);
                    copy(&mut out, 1 + (low >> 2) + (high << 6), count + 2)?;
                } else if t >= 16 {
                    let mut count = t & 7;
                    if count == 0 {
                        count = 7 + long(&mut ip)?;
                    }
                    let (low, high) = (next(&mut ip)?, next(&mut ip)?);
                    let distance = ((t & 8) << 11) + (low >> 2) + (high << 6);
                    // A distance of nothing ends the stream.
                    if distance == 0 {
                        break;
                    }
                    copy(&mut out, distance + 0x4000, count + 2)?;
                } else {
                    let distance = 1 + (t >> 2) + (next(&mut ip)? << 2);
                    copy(&mut out, distance, 2)?;
                }
                Step::Trailing(packed[ip - 2] as usize & 3)
            }
            // The last two bits of a match's code: so many literals after
            // it, and then a code that is a match whatever its value.
            Step::Trailing(0) => Step::Literals,
            Step::Trailing(t) => {
                literals(&mut out, &mut ip, t)?;
                Step::Match(next(&mut ip)?)
            }
        };
    }
    if out.len() != size {
        bail!("{} bytes unpacked where {size} are listed", out.len());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
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
}
