//! Swift `DiffLoupeTests/LineDiffTests.swift` の 1:1 移植
//! (LineDiffTests + TextFileLoaderTests)

use super::support::write_temp;
use diffloupe_core::line_diff::{self, DiffRowKind};
use diffloupe_core::options::ContentComparison;
use diffloupe_core::text_loader::{self, TextLoadResult};

mod diffloupe_tests {
    use super::*;

    #[test]
    fn test_identical_texts() {
        let cc = ContentComparison::default();
        let rows = line_diff::rows("a\nb\nc\n", "a\nb\nc\n", &cc).unwrap();
        assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
        assert_eq!(rows.len(), 4); // 末尾改行による空行を含む
    }

    #[test]
    fn test_addition() {
        let cc = ContentComparison::default();
        let rows = line_diff::rows("a\nb\n", "a\nx\nb\n", &cc).unwrap();
        let added: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Added)
            .collect();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].right_text.as_deref(), Some("x"));
        assert_eq!(added[0].left_number, None);
        assert_eq!(added[0].right_number, Some(2));
    }

    #[test]
    fn test_removal() {
        let cc = ContentComparison::default();
        let rows = line_diff::rows("a\nx\nb\n", "a\nb\n", &cc).unwrap();
        let removed: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Removed)
            .collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].left_text.as_deref(), Some("x"));
        assert_eq!(removed[0].right_number, None);
    }

    #[test]
    fn test_modification_pairing() {
        let cc = ContentComparison::default();
        let rows = line_diff::rows("a\nb\nc\n", "a\nX\nc\n", &cc).unwrap();
        let modified: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == DiffRowKind::Modified)
            .collect();
        assert_eq!(modified.len(), 1);
        assert_eq!(modified[0].left_text.as_deref(), Some("b"));
        assert_eq!(modified[0].right_text.as_deref(), Some("X"));
    }

    #[test]
    fn test_block_pairing_with_remainder() {
        // 2行削除+3行挿入 → 2行は「変更」ペア、1行は「追加」
        let cc = ContentComparison::default();
        let rows = line_diff::rows("1\n2\n3\n4\n", "1\nA\nB\nC\n4\n", &cc).unwrap();
        assert_eq!(
            rows.iter()
                .filter(|r| r.kind == DiffRowKind::Modified)
                .count(),
            2
        );
        assert_eq!(
            rows.iter().filter(|r| r.kind == DiffRowKind::Added).count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.kind == DiffRowKind::Removed)
                .count(),
            0
        );
    }

    #[test]
    fn test_crlf_treated_as_equal_lines() {
        let cc = ContentComparison::default();
        let rows = line_diff::rows("a\r\nb\r\n", "a\nb\n", &cc).unwrap();
        assert!(rows.iter().all(|r| r.kind == DiffRowKind::Context));
    }

    #[test]
    fn test_line_numbers_are_sequential() {
        let cc = ContentComparison::default();
        let rows = line_diff::rows("a\nb\nc\n", "a\nX\nY\nc\n", &cc).unwrap();
        let left_numbers: Vec<usize> = rows.iter().filter_map(|r| r.left_number).collect();
        let right_numbers: Vec<usize> = rows.iter().filter_map(|r| r.right_number).collect();
        assert_eq!(left_numbers, vec![1, 2, 3, 4]);
        assert_eq!(right_numbers, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_too_many_lines_returns_nil() {
        let cc = ContentComparison::default();
        let lines = "line\n".repeat(line_diff::MAX_TOTAL_LINES / 2 + 1);
        assert!(line_diff::rows(&lines, &lines, &cc).is_none());
    }
}

mod text_file_loader_tests {
    use super::*;

    #[test]
    fn test_utf8_text() {
        match text_loader::load(&write_temp("こんにちは\nworld\n".as_bytes())).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert!(t.contains("こんにちは"));
                assert_eq!(e, "UTF-8");
            }
            _ => panic!("テキストとして読めるべき"),
        }
    }

    #[test]
    fn test_null_byte_means_binary() {
        match text_loader::load(&write_temp(&[0x41, 0x00, 0x42])).unwrap() {
            TextLoadResult::Binary => {}
            _ => panic!("バイナリ判定されるべき"),
        }
    }

    #[test]
    fn test_invalid_utf8_means_binary() {
        // PNGヘッダ相当のバイト列(真のバイナリ)
        let png = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        ];
        match text_loader::load(&write_temp(&png)).unwrap() {
            TextLoadResult::Binary => {}
            _ => panic!("バイナリ判定されるべき"),
        }
    }

    #[test]
    fn test_empty_file_is_text() {
        match text_loader::load(&write_temp(&[])).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert_eq!(t, "");
                assert_eq!(e, "UTF-8");
            }
            _ => panic!("空ファイルはテキスト扱い"),
        }
    }

    #[test]
    fn test_shift_jis_text() {
        let original = "こんにちは、世界\nシフトJISのテスト";
        let data = encoding_rs::SHIFT_JIS.encode(original).0;
        match text_loader::load(&write_temp(&data)).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert_eq!(t, original);
                assert_eq!(e, "Shift_JIS");
            }
            _ => panic!("Shift_JISとして読めるべき"),
        }
    }

    #[test]
    fn test_eucjp_text() {
        let original = "これはEUC-JPのテストです\n日本語文字列";
        let data = encoding_rs::EUC_JP.encode(original).0;
        match text_loader::load(&write_temp(&data)).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert_eq!(t, original);
                assert_eq!(e, "EUC-JP");
            }
            _ => panic!("EUC-JPとして読めるべき"),
        }
    }

    #[test]
    fn test_iso2022jp_text() {
        let original = "日本語のISO-2022-JPテスト";
        let data = encoding_rs::ISO_2022_JP.encode(original).0;
        match text_loader::load(&write_temp(&data)).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert_eq!(t, original);
                assert_eq!(e, "ISO-2022-JP");
            }
            _ => panic!("ISO-2022-JPとして読めるべき"),
        }
    }

    #[test]
    fn test_utf16le_with_bom_text() {
        let original = "UTF-16LEのBOM付きテスト\n日本語";
        let mut data = vec![0xFF, 0xFE];
        for u in original.encode_utf16() {
            data.extend_from_slice(&u.to_le_bytes());
        }
        match text_loader::load(&write_temp(&data)).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert_eq!(t, original);
                assert_eq!(e, "UTF-16 LE");
            }
            _ => panic!("UTF-16 LEとして読めるべき"),
        }
    }

    #[test]
    fn test_utf8_with_bom_text() {
        let original = "UTF-8のBOM付きテスト\n日本語";
        let data = [vec![0xEF, 0xBB, 0xBF], original.as_bytes().to_vec()].concat();
        match text_loader::load(&write_temp(&data)).unwrap() {
            TextLoadResult::Text(t, e) => {
                assert_eq!(t, original);
                assert_eq!(e, "UTF-8 (BOM)");
            }
            _ => panic!("UTF-8 (BOM)として読めるべき"),
        }
    }
}
