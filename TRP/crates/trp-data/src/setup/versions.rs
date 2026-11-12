//! The versions an install states for itself. The game's client and SPT each write theirs into
//! their program's version resource: a Windows program's `VS_VERSIONINFO`, a documented format
//! (Microsoft's PE/COFF specification for the resource table that holds it; `VS_VERSIONINFO`,
//! `VS_FIXEDFILEINFO`, `StringFileInfo`, `StringTable` and `String` for the resource itself).
//! As read:
//! `EscapeFromTarkov.exe` names its product
//! `0.16.9.5-40743-f137e819` and its file 0.16.9.40743; SPT's server (`SPT_Runtime\SPT.Server.dll`)
//! and its client's core plugin (`BepInEx\plugins\spt\spt-core.dll`) both 4.1.6. SPT's own
//! configuration names the client it was made for (`core.json`, `compatibleTarkovVersion`,
//! 0.16.9.40743). Read only, as the rest of the install; the app compares (`trp-app`, `run_log`).

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::setup::install::Install;

/// A program's version resource: its fixed versions (four numbers each) and its text entries
/// (`ProductVersion`, `FileVersion` and the rest), in the order written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramVersion {
    pub file: [u16; 4],
    pub product: [u16; 4],
    pub strings: Vec<(String, String)>,
}

impl ProgramVersion {
    /// The version resource of the program at `path`.
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
        Self::of_program(&bytes).with_context(|| path.display().to_string())
    }

    /// The version resource of a program's bytes.
    pub fn of_program(bytes: &[u8]) -> Result<Self> {
        Self::of_resource(version_resource(bytes)?)
    }

    /// A version resource itself (`VS_VERSIONINFO`).
    pub fn of_resource(bytes: &[u8]) -> Result<Self> {
        let (root, _) = block(bytes)?;
        if root.key != "VS_VERSION_INFO" {
            bail!("the version resource begins with {:?}, not VS_VERSION_INFO", root.key);
        }
        let fixed = root.value;
        if u32_at(fixed, 0)? != 0xFEEF_04BD {
            bail!("the version resource has no fixed version (VS_FIXEDFILEINFO)");
        }
        let four = |high: u32, low: u32| [(high >> 16) as u16, high as u16, (low >> 16) as u16, low as u16];
        let file = four(u32_at(fixed, 8)?, u32_at(fixed, 12)?);
        let product = four(u32_at(fixed, 16)?, u32_at(fixed, 20)?);
        let mut strings = Vec::new();
        for info in children(root.children)?.into_iter().filter(|child| child.key == "StringFileInfo") {
            for table in children(info.children)? {
                for entry in children(table.children)? {
                    strings.push((entry.key, text(entry.value)));
                }
            }
        }
        Ok(Self { file, product, strings })
    }

    /// The text entry named `key`, where it says something.
    pub fn string(&self, key: &str) -> Option<&str> {
        self.strings.iter().find(|(name, _)| name == key).map(|(_, value)| value.trim()).filter(|value| !value.is_empty())
    }

    /// The product's version as the program writes it (`ProductVersion`), else its fixed one.
    pub fn product_text(&self) -> String {
        self.string("ProductVersion").map(str::to_owned).unwrap_or_else(|| dotted(self.product))
    }

    /// The file's version as the program writes it (`FileVersion`), else its fixed one.
    pub fn file_text(&self) -> String {
        self.string("FileVersion").map(str::to_owned).unwrap_or_else(|| dotted(self.file))
    }
}

fn dotted(numbers: [u16; 4]) -> String {
    numbers.map(|n| n.to_string()).join(".")
}

/// The leading numbers of a version as it is written: `0.16.9.5-40743-f137e819` is 0, 16, 9, 5;
/// `4.1.6-RELEASE+731d7a2` is 4, 1, 6.
pub fn numbers(version: &str) -> Vec<u32> {
    let lead: String = version.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    lead.split('.').map_while(|part| part.parse().ok()).collect()
}

/// Whether `found` is of the release `wanted` names: its leading numbers begin with all of
/// `wanted`'s (0.16.9.5 is of 0.16.9; 0.16.10 and 0.16.90 are not).
pub fn same_release(found: &str, wanted: &str) -> bool {
    let (found, wanted) = (numbers(found), numbers(wanted));
    !wanted.is_empty() && found.len() >= wanted.len() && found[..wanted.len()] == wanted[..]
}

impl Install {
    /// The game's client.
    pub fn game_program(&self) -> PathBuf {
        self.root.join("EscapeFromTarkov.exe")
    }

    /// SPT's server, whose database the app reads.
    pub fn spt_server_program(&self) -> PathBuf {
        self.root.join("SPT_Runtime").join("SPT.Server.dll")
    }

    /// SPT's core plugin of the client.
    pub fn spt_client_program(&self) -> PathBuf {
        self.root.join("BepInEx").join("plugins").join("spt").join("spt-core.dll")
    }

    /// The game client's version, as its program names its product.
    pub fn game_version(&self) -> Result<String> {
        Ok(ProgramVersion::read(&self.game_program())?.product_text())
    }

    /// SPT's version, as its server's program names its file.
    pub fn spt_version(&self) -> Result<String> {
        Ok(ProgramVersion::read(&self.spt_server_program())?.file_text())
    }

    /// The version of SPT's plugin in the client, as its program names its file.
    pub fn spt_client_version(&self) -> Result<String> {
        Ok(ProgramVersion::read(&self.spt_client_program())?.file_text())
    }

    /// The game's client SPT was made for, as SPT's configuration names it.
    pub fn spt_made_for(&self) -> Result<String> {
        let path = self.root.join("SPT_Runtime").join("SPT_Data").join("configs").join("core.json");
        let text = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text).with_context(|| path.display().to_string())?;
        value["compatibleTarkovVersion"].as_str().map(str::to_owned).with_context(|| format!("{} names no compatibleTarkovVersion", path.display()))
    }
}

// ==== A program's resource table ====

fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    let pair = bytes.get(at..at + 2).with_context(|| format!("it ends before byte {at}"))?;
    Ok(u16::from_le_bytes([pair[0], pair[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    let four = bytes.get(at..at + 4).with_context(|| format!("it ends before byte {at}"))?;
    Ok(u32::from_le_bytes([four[0], four[1], four[2], four[3]]))
}

/// The resource kind of a version (`RT_VERSION`).
const VERSION_KIND: u32 = 16;

/// A program's version resource, found through its resource table: the table's place among
/// the optional header's data directories (the third), then its tree of three levels, the kind,
/// the name and the language, taking the version's kind and the first name and language.
fn version_resource(file: &[u8]) -> Result<&[u8]> {
    if file.get(0..2) != Some(b"MZ") {
        bail!("not a Windows program (it does not begin with MZ)");
    }
    let pe = u32_at(file, 0x3C)? as usize;
    if file.get(pe..pe + 4) != Some(b"PE\0\0") {
        bail!("not a Windows program (no PE signature)");
    }
    let coff = pe + 4;
    let sections = u16_at(file, coff + 2)? as usize;
    let optional_size = u16_at(file, coff + 16)? as usize;
    let optional = coff + 20;
    // The data directories follow the optional header's fixed part, whose last field counts them.
    let directories = match u16_at(file, optional)? {
        0x10B => optional + 96,
        0x20B => optional + 112,
        magic => bail!("an optional header of an unknown kind ({magic:#x})"),
    };
    if u32_at(file, directories - 4)? < 3 {
        bail!("the program has no resource table");
    }
    let table_address = u32_at(file, directories + 2 * 8)?;
    if table_address == 0 {
        bail!("the program has no resources");
    }
    let section_table = optional + optional_size;
    // Where in the file an address of the loaded program lies: in the section that holds it.
    let offset_of = |address: u32| -> Result<usize> {
        for index in 0..sections {
            let section = section_table + index * 40;
            let size = u32_at(file, section + 8)?.max(u32_at(file, section + 16)?);
            let start = u32_at(file, section + 12)?;
            let raw = u32_at(file, section + 20)?;
            if address >= start && (address as u64) < start as u64 + size as u64 {
                return Ok((address - start) as usize + raw as usize);
            }
        }
        bail!("no section of the program holds the address {address:#x}")
    };
    let root = offset_of(table_address)?;
    let entry_of = |directory: usize, kind: Option<u32>| -> Result<u32> {
        let count = u16_at(file, directory + 12)? as usize + u16_at(file, directory + 14)? as usize;
        for index in 0..count {
            let entry = directory + 16 + index * 8;
            if kind.is_none_or(|kind| u32_at(file, entry).is_ok_and(|id| id == kind)) {
                return u32_at(file, entry + 4);
            }
        }
        bail!("the program has no version resource")
    };
    let mut directory = root;
    for kind in [Some(VERSION_KIND), None] {
        let below = entry_of(directory, kind)?;
        if below & 0x8000_0000 == 0 {
            bail!("the program's resource table ends early");
        }
        directory = root + (below & 0x7FFF_FFFF) as usize;
    }
    let data = entry_of(directory, None)?;
    if data & 0x8000_0000 != 0 {
        bail!("the program's resource table goes deeper than three levels");
    }
    let data = root + data as usize;
    let at = offset_of(u32_at(file, data)?)?;
    let size = u32_at(file, data + 4)? as usize;
    file.get(at..at + size).context("the version resource runs past the program's end")
}

// ==== The version resource's blocks ====

/// One block of a version resource: its key, its value and the blocks within it.
struct Block<'a> {
    key: String,
    value: &'a [u8],
    children: &'a [u8],
}

fn align(at: usize) -> usize {
    (at + 3) & !3
}

/// The block at the start of `bytes`, and how far the next one is: its length (which leaves
/// out the padding after it), its value's length (in characters for a text value, in bytes for
/// a binary one), its kind (1 text, 0 binary), its key (UTF-16, ended by a zero), then, each on a
/// boundary of four bytes, its value and the blocks within it.
fn block(bytes: &[u8]) -> Result<(Block<'_>, usize)> {
    let length = u16_at(bytes, 0)? as usize;
    let value_length = u16_at(bytes, 2)? as usize;
    let text_value = u16_at(bytes, 4)? == 1;
    if length < 6 {
        bail!("a block of the version resource of {length} bytes");
    }
    let body = bytes.get(..length).context("a block runs past the version resource's end")?;
    let mut at = 6;
    let mut key = Vec::new();
    loop {
        let unit = u16_at(body, at).context("a block's key runs past the block")?;
        at += 2;
        if unit == 0 {
            break;
        }
        key.push(unit);
    }
    let value_start = align(at).min(length);
    let value_end = (value_start + if text_value { value_length * 2 } else { value_length }).min(length);
    let block = Block { key: String::from_utf16_lossy(&key), value: &body[value_start..value_end], children: &body[align(value_end).min(length)..] };
    Ok((block, align(length)))
}

/// The blocks one after another in `bytes`.
fn children(mut bytes: &[u8]) -> Result<Vec<Block<'_>>> {
    let mut found = Vec::new();
    while bytes.len() >= 6 && u16_at(bytes, 0)? != 0 {
        let (child, step) = block(bytes)?;
        found.push(child);
        bytes = bytes.get(step..).unwrap_or(&[]);
    }
    Ok(found)
}

/// A text value: UTF-16, up to its zero.
fn text(value: &[u8]) -> String {
    let units: Vec<u16> = value.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).take_while(|&unit| unit != 0).collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block as the format lays it out: its length, its value's, its kind, its key, then the
    /// value and the blocks within, each on a boundary of four bytes.
    fn block_of(key: &str, text_value: bool, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
        let pad = |bytes: &mut Vec<u8>| bytes.resize(align(bytes.len()), 0);
        let mut bytes = vec![0; 6];
        key.encode_utf16().chain([0]).for_each(|unit| bytes.extend(unit.to_le_bytes()));
        pad(&mut bytes);
        bytes.extend(value);
        for child in children {
            pad(&mut bytes);
            bytes.extend(child);
        }
        let value_length = if text_value { value.len() / 2 } else { value.len() };
        let length = bytes.len() as u16;
        bytes[0..2].copy_from_slice(&length.to_le_bytes());
        bytes[2..4].copy_from_slice(&(value_length as u16).to_le_bytes());
        bytes[4..6].copy_from_slice(&(text_value as u16).to_le_bytes());
        bytes
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn a_version_resource_read() {
        let mut fixed = vec![0u8; 52];
        let words: [u32; 6] = [0xFEEF_04BD, 0x0001_0000, 0x0000_0010, 0x0009_9F27, 0x0000_0010, 0x0009_0005];
        for (index, word) in words.iter().enumerate() {
            fixed[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        let strings = block_of(
            "StringFileInfo",
            true,
            &[],
            &[block_of("040904b0", true, &[], &[block_of("FileVersion", true, &utf16("0.16.9.40743"), &[]), block_of("ProductVersion", true, &utf16("0.16.9.5-40743-x"), &[])])],
        );
        let resource = block_of("VS_VERSION_INFO", false, &fixed, &[strings, block_of("VarFileInfo", true, &[], &[])]);
        let version = ProgramVersion::of_resource(&resource).unwrap();
        assert_eq!(version.file, [0, 16, 9, 40743]);
        assert_eq!(version.product, [0, 16, 9, 5]);
        assert_eq!(version.product_text(), "0.16.9.5-40743-x");
        assert_eq!(version.file_text(), "0.16.9.40743");
        assert_eq!(version.string("CompanyName"), None);
    }

    #[test]
    fn releases_compared_by_their_leading_numbers() {
        assert_eq!(numbers("0.16.9.5-40743-f137e819"), [0, 16, 9, 5]);
        assert_eq!(numbers("4.1.6-RELEASE+731d7a2"), [4, 1, 6]);
        assert!(same_release("0.16.9.5-40743-f137e819", "0.16.9"));
        assert!(same_release("4.1.6", "4.1"));
        assert!(same_release("4.1", "4.1"));
        assert!(!same_release("0.16.10.1", "0.16.9"));
        assert!(!same_release("0.16.90", "0.16.9"));
        assert!(!same_release("4", "4.1"));
        assert!(!same_release("unknown", "4.1"));
        assert!(!same_release("4.1.6", ""));
    }

    #[test]
    fn not_a_program() {
        assert!(ProgramVersion::of_program(b"not a program").is_err());
        assert!(ProgramVersion::of_program(&[]).is_err());
    }
}
