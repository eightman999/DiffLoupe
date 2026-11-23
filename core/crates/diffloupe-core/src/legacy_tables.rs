//! 追加文字コード用の表(開発時に生成して repo に入れたバイナリ)。ビルド時に外部へ取りに行かない。
//!
//! 生成: `cd core && python3 -I tools/gen_text_encoding_tables.py`(macOS 上で完結。生成元は
//! Python の codecs と Apple の libicucore / CoreFoundation。詳細はスクリプト先頭)。
//!
//! - `jisx0213.bin`: JIS X 0213:2004 の面 1・面 2(各 94 区 x 94 点、区点順)。`u32` LE の配列で、
//!   値 0 は未割当。下位 24 ビットが 1 つ目の符号位置、上位 8 ビットが 2 つ目の符号位置
//!   (結合文字)の番号(0 なし、1 = U+309A、2 = U+0300、3 = U+0301、4 = U+02E5、5 = U+02E9)。
//! - `ebcdic_sbcs.bin`: 256 要素の `u16` LE 表を IBM037 / IBM500 / IBM1140 / IBM273 / IBM290 /
//!   IBM930(単バイト部)/ IBM939(単バイト部)の順に並べたもの。0xFFFF は未割当。
//! - `ibm930_dbcs.bin` / `ibm939_dbcs.bin` / `macjapanese.bin`: コード昇順の疎な表。1 件 10 バイトで、
//!   コード `u16`、符号位置 1 `u32`、符号位置 2 `u32`(0 なし)(すべて LE)。IBM930 / IBM939 の
//!   コードは SO(0x0E)後の 2 バイト、MacJapanese は 1 バイト(0xFF 以下)または 2 バイト。
//!   MacJapanese の符号位置 2 が U+F87A〜U+F87F のものは Apple の変換ヒント(私用領域)で、
//!   読み込み時は無視する。

pub(crate) static JISX0213: &[u8] = include_bytes!("legacy_tables/jisx0213.bin");
pub(crate) static EBCDIC_SBCS: &[u8] = include_bytes!("legacy_tables/ebcdic_sbcs.bin");
pub(crate) static IBM930_DBCS: &[u8] = include_bytes!("legacy_tables/ibm930_dbcs.bin");
pub(crate) static IBM939_DBCS: &[u8] = include_bytes!("legacy_tables/ibm939_dbcs.bin");
pub(crate) static MACJAPANESE: &[u8] = include_bytes!("legacy_tables/macjapanese.bin");

/// 疎な表の 1 件
#[derive(Clone, Copy)]
pub(crate) struct Record {
    pub code: u16,
    pub cp1: u32,
    pub cp2: u32,
}

pub(crate) fn parse_records(blob: &'static [u8]) -> Vec<Record> {
    blob.as_chunks::<10>()
        .0
        .iter()
        .map(|c| Record {
            code: u16::from_le_bytes([c[0], c[1]]),
            cp1: u32::from_le_bytes([c[2], c[3], c[4], c[5]]),
            cp2: u32::from_le_bytes([c[6], c[7], c[8], c[9]]),
        })
        .collect()
}
