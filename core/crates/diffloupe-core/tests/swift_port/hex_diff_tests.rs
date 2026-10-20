//! Swift `DiffLoupeTests/HexDiffTests.swift` と `CoreTests/HexDiffTests.swift` の 1:1 移植

/// DiffLoupeTests/HexDiffTests.swift(18件)
mod diffloupe_tests {
    use diffloupe_core::hex_diff;
    use std::path::PathBuf;

    fn cap() -> usize {
        hex_diff::DEFAULT_BYTE_CAP
    }

    // MARK: - 等値

    #[test]
    fn test_identical_bytes_are_equal() {
        let left = [0x00, 0x01, 0x02, 0xFF, 0x41];
        let result = hex_diff::compare(&left, &left, cap());
        assert!(result.is_equal());
        assert!(!result.truncated);
        assert_eq!(result.row_count(), 1);
        assert!(result.row(0).differing_byte_offsets.is_empty());
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(hex_diff::BYTES_PER_ROW, 16);
        assert_eq!(hex_diff::DEFAULT_BYTE_CAP, 8 * 1024 * 1024);
    }

    // MARK: - バイト変更

    #[test]
    fn test_changed_byte() {
        let left = [0x41, 0x42, 0x43, 0x44, 0x45, 0x46];
        let right = [0x41, 0x42, 0x58, 0x44, 0x45, 0x46];
        let result = hex_diff::compare(&left, &right, cap());
        assert!(!result.is_equal());
        let row = result.row(0);
        assert_eq!(row.differing_byte_offsets, vec![2]);
        assert_eq!(row.left_bytes.as_ref().unwrap()[2], 0x43);
        assert_eq!(row.right_bytes.as_ref().unwrap()[2], 0x58);
    }

    #[test]
    fn test_changed_bytes_across_multiple_rows() {
        let left: Vec<u8> = (0..40).collect();
        let mut right = left.clone();
        right[16] = 0xFF;
        right[17] = 0xFE;
        let result = hex_diff::compare(&left, &right, cap());
        assert_eq!(result.row_count(), 3);
        assert_eq!(result.row(0).differing_byte_offsets, Vec::<usize>::new());
        assert_eq!(result.row(1).differing_byte_offsets, vec![0, 1]);
        assert_eq!(result.row(2).differing_byte_offsets, Vec::<usize>::new());
        assert_eq!(result.row(1).offset, 16);
    }

    #[test]
    fn test_row_exposes_bytes_and_ascii() {
        let result = hex_diff::compare(&[0x41, 0x42, 0x43], &[0x41, 0x58, 0x43], cap());
        let row = result.row(0);
        assert_eq!(row.offset, 0);
        assert_eq!(row.left_bytes.as_deref(), Some(&[0x41, 0x42, 0x43][..]));
        assert_eq!(row.right_bytes.as_deref(), Some(&[0x41, 0x58, 0x43][..]));
        assert_eq!(row.left_ascii.as_deref(), Some("ABC"));
        assert_eq!(row.right_ascii.as_deref(), Some("AXC"));
        assert_eq!(row.differing_byte_offsets, vec![1]);
    }

    // MARK: - 挿入/削除(サイズ差のあるシフト)

    #[test]
    fn test_insertion_shifts_remainder() {
        let left = [0x41, 0x42, 0x43, 0x44, 0x45, 0x46];
        let right = [0x41, 0x42, 0x58, 0x43, 0x44, 0x45, 0x46];
        let result = hex_diff::compare(&left, &right, cap());
        assert!(!result.is_equal());
        assert_eq!(result.row(0).differing_byte_offsets, vec![2, 3, 4, 5]);
    }

    #[test]
    fn test_deletion_shifts_remainder() {
        let left = [0x41, 0x42, 0x58, 0x43, 0x44, 0x45, 0x46];
        let right = [0x41, 0x42, 0x43, 0x44, 0x45, 0x46];
        let result = hex_diff::compare(&left, &right, cap());
        assert!(!result.is_equal());
        assert_eq!(result.row(0).differing_byte_offsets, vec![2, 3, 4, 5]);
    }

    #[test]
    fn test_unequal_lengths_shorter_side_becomes_nil() {
        let left: Vec<u8> = (0..20).collect();
        let right: Vec<u8> = (0..12).collect();
        let result = hex_diff::compare(&left, &right, cap());
        assert!(!result.is_equal());
        assert_eq!(result.row_count(), 2);
        let row0 = result.row(0);
        assert!(row0.left_bytes.is_some());
        assert!(row0.right_bytes.is_some());
        assert!(row0.differing_byte_offsets.is_empty());
        let row1 = result.row(1);
        assert_eq!(row1.offset, 16);
        assert_eq!(row1.left_bytes.as_deref(), Some(&[16, 17, 18, 19][..]));
        assert!(row1.right_bytes.is_none());
        assert!(row1.left_ascii.is_some());
        assert!(row1.right_ascii.is_none());
    }

    // MARK: - 空入力

    #[test]
    fn test_empty_inputs_are_equal() {
        let result = hex_diff::compare(&[], &[], cap());
        assert!(result.is_equal());
        assert_eq!(result.row_count(), 0);
        assert!(!result.truncated);
    }

    #[test]
    fn test_empty_left_against_non_empty() {
        let result = hex_diff::compare(&[], &[0x01, 0x02], cap());
        assert!(!result.is_equal());
        assert_eq!(result.row_count(), 1);
        let row = result.row(0);
        assert!(row.left_bytes.is_none());
        assert!(row.right_bytes.is_some());
        assert!(row.differing_byte_offsets.is_empty());
    }

    // MARK: - ASCII表現

    #[test]
    fn test_ascii_representation() {
        assert_eq!(
            hex_diff::ascii(&[0x41, 0x42, 0x0A, 0x00, 0x7F, 0x20]),
            "AB... "
        );
        assert_eq!(hex_diff::ascii(&[0x1F, 0x7E, 0xFF]), ".~.");
    }

    // MARK: - 上限読み込み(truncated)

    #[test]
    fn test_explicit_cap_truncates_larger_files() {
        let big = vec![0xABu8; 100];
        let mut changed = big.clone();
        changed[50] = 0x01;
        let result = hex_diff::compare(&big, &changed, 64);
        assert!(result.truncated);
        assert_eq!(result.left_data.len(), 64);
        assert_eq!(result.right_data.len(), 64);
        assert_eq!(result.row_count(), 4);
        let row = result.row(3);
        assert_eq!(row.offset, 48);
        assert_eq!(row.differing_byte_offsets, vec![2]);
        assert_eq!(row.left_bytes.as_ref().unwrap()[2], 0xAB);
        assert_eq!(row.right_bytes.as_ref().unwrap()[2], 0x01);
    }

    #[test]
    fn test_default_cap_truncates_at_eight_mi_b() {
        let big = vec![0x00u8; hex_diff::DEFAULT_BYTE_CAP + 1];
        let result = hex_diff::compare(&big, &big, cap());
        assert!(result.truncated);
        assert_eq!(result.left_data.len(), hex_diff::DEFAULT_BYTE_CAP);
        assert_eq!(result.right_data.len(), hex_diff::DEFAULT_BYTE_CAP);
        // truncated では完全一致と判定しない
        assert!(!result.is_equal());
    }

    #[test]
    fn test_truncated_same_size_difference_beyond_cap_is_not_equal() {
        let left = vec![0xABu8; 100];
        let mut right = left.clone();
        right[80] = 0x01; // cap(64)を超えた位置の差分
        let result = hex_diff::compare(&left, &right, 64);
        assert!(result.truncated);
        assert_eq!(result.left_data, result.right_data);
        assert!(!result.is_equal());
    }

    #[test]
    fn test_exact_cap_size_is_not_truncated() {
        let data = vec![0x00u8; 64];
        let result = hex_diff::compare(&data, &data, 64);
        assert!(!result.truncated);
        assert_eq!(result.left_data.len(), 64);
    }

    // MARK: - URL読み込み

    fn make_temp_pair() -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "HexDiffTests-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.clone(), dir.join("left.bin"), dir.join("right.bin"))
    }

    #[test]
    fn test_url_compare_equal_files() {
        let (dir, left, right) = make_temp_pair();
        std::fs::write(&left, [0x00, 0x01, 0x02]).unwrap();
        std::fs::write(&right, [0x00, 0x01, 0x02]).unwrap();
        let result = hex_diff::compare_urls(&left, &right, cap()).unwrap();
        assert!(result.is_equal());
        assert_eq!(result.left_total_size, 3);
        assert_eq!(result.right_total_size, 3);
        assert!(!result.truncated);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_url_compare_reports_total_sizes_beyond_cap() {
        let (dir, left, right) = make_temp_pair();
        std::fs::write(&left, vec![0xABu8; 100]).unwrap();
        std::fs::write(&right, vec![0xABu8; 100]).unwrap();
        let result = hex_diff::compare_urls(&left, &right, 64).unwrap();
        assert!(result.truncated);
        assert_eq!(result.left_total_size, 100);
        assert_eq!(result.right_total_size, 100);
        assert_eq!(result.left_data.len(), 64);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_url_missing_file_throws() {
        let url = PathBuf::from(format!(
            "/nonexistent/diffloupe-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(hex_diff::compare_urls(&url, &url, cap()).is_err());
    }
}

/// CoreTests/HexDiffTests.swift(9件)
mod core_tests {
    use diffloupe_core::hex_diff;

    fn cap() -> usize {
        hex_diff::DEFAULT_BYTE_CAP
    }

    #[test]
    fn test_identical_bytes_are_equal() {
        let result = hex_diff::compare(&[0x00, 0x01, 0x02], &[0x00, 0x01, 0x02], cap());
        assert!(result.is_equal());
        assert_eq!(result.row_count(), 1);
        assert!(result.row(0).differing_byte_offsets.is_empty());
    }

    #[test]
    fn test_changed_byte_is_reported_per_row() {
        let result = hex_diff::compare(&[0x41, 0x42, 0x43], &[0x41, 0x58, 0x43], cap());
        assert!(!result.is_equal());
        assert_eq!(result.row(0).differing_byte_offsets, vec![1]);
        assert_eq!(result.row(0).left_bytes.as_ref().unwrap()[1], 0x42);
        assert_eq!(result.row(0).right_bytes.as_ref().unwrap()[1], 0x58);
    }

    #[test]
    fn test_insert_and_delete_shift_remainder() {
        let left = [0x41, 0x42, 0x43, 0x44, 0x45];
        let inserted = [0x41, 0x42, 0x58, 0x43, 0x44, 0x45];
        let deleted = [0x43, 0x44, 0x45];
        assert_eq!(
            hex_diff::compare(&left, &inserted, cap())
                .row(0)
                .differing_byte_offsets,
            vec![2, 3, 4]
        );
        assert_eq!(
            hex_diff::compare(&left, &deleted, cap())
                .row(0)
                .differing_byte_offsets,
            vec![0, 1, 2]
        );
    }

    #[test]
    fn test_unequal_lengths_shorter_side_becomes_nil() {
        let left: Vec<u8> = (0..20).collect();
        let right: Vec<u8> = (0..12).collect();
        let result = hex_diff::compare(&left, &right, cap());
        assert!(!result.is_equal());
        assert_eq!(result.row_count(), 2);
        assert!(result.row(0).differing_byte_offsets.is_empty());
        assert_eq!(result.row(1).offset, 16);
        assert!(result.row(1).right_bytes.is_none());
    }

    #[test]
    fn test_empty_inputs_are_equal() {
        let result = hex_diff::compare(&[], &[], cap());
        assert!(result.is_equal());
        assert_eq!(result.row_count(), 0);
        assert!(!result.truncated);
    }

    #[test]
    fn test_bounded_reads_report_truncation() {
        let big = vec![0xABu8; 100];
        let mut changed = big.clone();
        changed[50] = 0x01;
        let result = hex_diff::compare(&big, &changed, 64);
        assert!(result.truncated);
        assert_eq!(result.left_data.len(), 64);
        assert_eq!(result.right_data.len(), 64);
        assert_eq!(result.row_count(), 4);
        assert_eq!(result.row(3).offset, 48);
        assert_eq!(result.row(3).differing_byte_offsets, vec![2]);
    }

    #[test]
    fn test_exact_cap_size_is_not_truncated() {
        let data = vec![0x00u8; 64];
        assert!(!hex_diff::compare(&data, &data, 64).truncated);
    }

    #[test]
    fn test_truncated_same_size_difference_beyond_cap_is_not_equal() {
        let left = vec![0xABu8; 100];
        let mut right = left.clone();
        right[80] = 0x01; // cap(64)を超えた位置の差分
        let result = hex_diff::compare(&left, &right, 64);
        assert!(result.truncated);
        assert_eq!(result.left_data, result.right_data);
        assert!(!result.is_equal());
    }

    #[test]
    fn test_default_cap_is_eight_mi_b() {
        assert_eq!(hex_diff::DEFAULT_BYTE_CAP, 8 * 1024 * 1024);
        assert_eq!(hex_diff::BYTES_PER_ROW, 16);
    }
}
