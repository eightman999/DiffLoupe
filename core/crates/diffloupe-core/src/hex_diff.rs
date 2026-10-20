//! Hex バイナリ比較(Swift `HexDiff` 相当)

use std::path::Path;

pub const BYTES_PER_ROW: usize = 16;
pub const DEFAULT_BYTE_CAP: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct HexRow {
    pub offset: usize,
    pub left_bytes: Option<Vec<u8>>,
    pub right_bytes: Option<Vec<u8>>,
    pub left_ascii: Option<String>,
    pub right_ascii: Option<String>,
    pub differing_byte_offsets: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct HexComparison {
    pub left_data: Vec<u8>,
    pub right_data: Vec<u8>,
    pub left_total_size: i64,
    pub right_total_size: i64,
    pub truncated: bool,
}

impl HexComparison {
    pub fn bytes_per_row(&self) -> usize {
        BYTES_PER_ROW
    }
    pub fn row_count(&self) -> usize {
        let max = self.left_data.len().max(self.right_data.len());
        max.div_ceil(BYTES_PER_ROW)
    }
    pub fn is_equal(&self) -> bool {
        !self.truncated
            && self.left_total_size == self.right_total_size
            && self.left_data == self.right_data
    }
    pub fn row(&self, index: usize) -> HexRow {
        let rc = self.row_count();
        let safe = index.min(rc.saturating_sub(1)).max(0);
        let offset = safe * BYTES_PER_ROW;
        let left_end = (offset + BYTES_PER_ROW).min(self.left_data.len());
        let right_end = (offset + BYTES_PER_ROW).min(self.right_data.len());
        let left_bytes = if left_end > offset {
            Some(self.left_data[offset..left_end].to_vec())
        } else {
            None
        };
        let right_bytes = if right_end > offset {
            Some(self.right_data[offset..right_end].to_vec())
        } else {
            None
        };
        let common = left_end.min(right_end).saturating_sub(offset);
        let mut differing = Vec::new();
        for i in 0..common {
            if self.left_data[offset + i] != self.right_data[offset + i] {
                differing.push(i);
            }
        }
        HexRow {
            offset,
            left_ascii: left_bytes.as_ref().map(|b| ascii(b)),
            right_ascii: right_bytes.as_ref().map(|b| ascii(b)),
            left_bytes,
            right_bytes,
            differing_byte_offsets: differing,
        }
    }
}

pub fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x20..=0x7E).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

pub fn compare_urls(left: &Path, right: &Path, max_bytes: usize) -> std::io::Result<HexComparison> {
    let (ld, lt) = read_bounded(left, max_bytes)?;
    let (rd, rt) = read_bounded(right, max_bytes)?;
    Ok(make_comparison(ld, rd, lt, rt, max_bytes))
}

pub fn compare(left: &[u8], right: &[u8], max_bytes: usize) -> HexComparison {
    make_comparison(
        left.to_vec(),
        right.to_vec(),
        left.len() as i64,
        right.len() as i64,
        max_bytes,
    )
}

fn make_comparison(
    left: Vec<u8>,
    right: Vec<u8>,
    lt: i64,
    rt: i64,
    max_bytes: usize,
) -> HexComparison {
    let cap = max_bytes;
    let lb = if left.len() > cap {
        left[..cap].to_vec()
    } else {
        left
    };
    let rb = if right.len() > cap {
        right[..cap].to_vec()
    } else {
        right
    };
    let ltr = if lt >= 0 {
        lt > cap as i64
    } else {
        lb.len() == cap && cap > 0
    };
    let rtr = if rt >= 0 {
        rt > cap as i64
    } else {
        rb.len() == cap && cap > 0
    };
    HexComparison {
        left_data: lb,
        right_data: rb,
        left_total_size: lt,
        right_total_size: rt,
        truncated: ltr || rtr,
    }
}

fn read_bounded(path: &Path, max_bytes: usize) -> std::io::Result<(Vec<u8>, i64)> {
    let total = std::fs::metadata(path)
        .map(|m| m.len() as i64)
        .unwrap_or(-1);
    let read_count = if total >= 0 {
        (total as usize).min(max_bytes)
    } else {
        max_bytes
    };
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut data = vec![0u8; read_count];
    let mut read = 0usize;
    while read < read_count {
        let n = f.read(&mut data[read..])?;
        if n == 0 {
            break;
        }
        read += n;
    }
    data.truncate(read);
    Ok((data, total))
}
