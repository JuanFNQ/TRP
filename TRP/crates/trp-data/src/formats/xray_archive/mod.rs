//! The X-Ray engine's packed archives, where the other game keeps the models and pictures not loose.
//! The `.db0`, `.db1`, ... files. Read in place;
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

mod lzhuf;
mod lzo1x;

pub use lzhuf::*;
pub use lzo1x::*;

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

#[cfg(test)]
mod tests;
