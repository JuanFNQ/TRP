//! Unpacking LZHUF: the adaptive Huffman tree and the bit reader.

use super::*;

/// The window, the longest match, the shortest one worth a code.
pub(super) const WINDOW: usize = 4096;

pub(super) const LONGEST: usize = 60;

pub(super) const SHORTEST: usize = 2;

/// The codes: 256 bytes and the lengths of a match; the tree's nodes; its
/// root; the count at which the tree is rebuilt with every count halved.
const CODES: usize = 256 - SHORTEST + LONGEST;

pub(super) const NODES: usize = CODES * 2 - 1;

pub(super) const ROOT: usize = NODES - 1;

/// The engine's is half the 1988 program's (0x8000): read from the
/// archives themselves, every table of the 68 decoding whole
/// at this and the twelve longer ones going wrong at the other, where the
/// tree is first rebuilt.
const MOST: u32 = 0x4000;

/// The length of the code of a match's distance's upper six bits, for
/// each of their 64 values: the codes themselves are given out in order,
/// each the one before and one more at its own length.
pub(super) const POSITION_LENGTH: [u8; 64] = {
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
pub(super) struct Tree {
    pub(super) count: Vec<u32>,
    child: Vec<usize>,
    pub(super) parent: Vec<usize>,
}

impl Tree {
    pub(super) fn new() -> Self {
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
    pub(super) fn update(&mut self, code: usize) {
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
pub(super) struct Bits<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) at: usize,
    held: u32,
    pub(super) count: u32,
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

    pub(super) fn bit(&mut self) -> usize {
        self.fill();
        let bit = (self.held >> 15) & 1;
        self.held = (self.held << 1) & 0xFFFF;
        self.count -= 1;
        bit as usize
    }

    pub(super) fn byte(&mut self) -> usize {
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
