// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: バイナリ比較用の16バイト行Hex差分エンジン。ファイル全体をメモリに読み込まず、
// 上限バイト数(既定8MiB)までを安全に読み込み、行単位の差分情報とASCII表示を提供する(SPEC §2.6拡張)
// UIは表示行を必要時に生成するため、このエンジンが全行を事前に保持しないこと。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

import Foundation

/// 16バイト行の1行分。そのオフセットにデータが存在しない側は bytes/ASCII を nil にする
/// (サイズ差による欠損を表す)。行内でバイト数が足りない(ファイル末尾)場合は不足分を欠損として扱う。
public struct HexRow: Equatable, Sendable {
    public let offset: Int
    public let leftBytes: [UInt8]?
    public let rightBytes: [UInt8]?
    public let leftASCII: String?
    public let rightASCII: String?
    /// 行内で左右が異なるバイトの行内インデックス(0..<bytesPerRow)。一致バイトは含まない。
    /// 片側にのみ存在するバイトは「差分」ではなく欠損として扱うため含まれない。
    public let differingByteOffsets: [Int]
}

/// バイナリ比較の結果。読み込んだ上限分の Data を保持し、行は必要時に生成する
/// (8MiB上限でも行数は最大52万行超になるため、全行を事前生成してメモリに保持しない)。
public struct HexComparison: Sendable {
    public let leftData: Data
    public let rightData: Data
    /// ファイルの総サイズ(読み込み上限前の実際のサイズ)。取得不能時は -1
    public let leftTotalSize: Int64
    public let rightTotalSize: Int64
    /// 片側でも総サイズが上限を超えた場合に true(超えた分は比較対象外)
    public let truncated: Bool
    public let bytesPerRow: Int = HexDiff.bytesPerRow

    /// 行数(サイズの大きい方のバイト数から算出)。空同士なら 0
    public var rowCount: Int {
        let maxBytes = max(leftData.count, rightData.count)
        return (maxBytes + bytesPerRow - 1) / bytesPerRow
    }

    /// 左右が完全に一致しているか(総サイズ一致かつ読み込んだ範囲が一致)。空同士は true。
    /// truncated(読み込み上限で打ち切られた)場合は上限を超えた部分が不明のため、決して完全一致とは判定しない
    public var isEqual: Bool {
        !truncated && leftTotalSize == rightTotalSize && leftData == rightData
    }

    /// 行インデックスに対応する行を返す。範囲外インデックスは安全な範囲へクランプする
    public func row(at index: Int) -> HexRow {
        let safeIndex = min(max(index, 0), max(rowCount - 1, 0))
        let offset = safeIndex * bytesPerRow
        let leftEnd = min(offset + bytesPerRow, leftData.count)
        let rightEnd = min(offset + bytesPerRow, rightData.count)

        let leftBytes = leftEnd > offset ? [UInt8](leftData[offset..<leftEnd]) : nil
        let rightBytes = rightEnd > offset ? [UInt8](rightData[offset..<rightEnd]) : nil

        // 両側に存在するバイトのみ「差分」の対象とする
        let commonCount = min(leftEnd, rightEnd) - offset
        var differing: [Int] = []
        if commonCount > 0 {
            for i in 0..<commonCount where leftData[offset + i] != rightData[offset + i] {
                differing.append(i)
            }
        }

        return HexRow(
            offset: offset,
            leftBytes: leftBytes,
            rightBytes: rightBytes,
            leftASCII: leftBytes.map(HexDiff.ascii),
            rightASCII: rightBytes.map(HexDiff.ascii),
            differingByteOffsets: differing
        )
    }
}

/// Rust 側の読み込み失敗をラップするエラー
struct HexLoadIOError: LocalizedError, Sendable {
    let message: String
    var errorDescription: String? { message }
}

public enum HexDiff {

    /// 1行あたりのバイト数(16バイト行)
    public static let bytesPerRow = 16

    /// 比較に読み込む既定の上限バイト数(8MiB)。これを超えるファイルは先頭分のみ比較し truncated で通知する
    public static let defaultByteCap = 8 * 1024 * 1024

    /// ファイル同士を上限バイト数まで比較する。上限超過時は truncated が true になる。
    /// - Throws: ファイルの読み込みエラー(存在しない・アクセス不可など)
    public static func compare(leftURL: URL, rightURL: URL, maxBytes: Int = defaultByteCap) throws -> HexComparison {
        let ffi: FfiHexComparison
        do {
            ffi = try diffloupeHexCompareFiles(
                leftPath: leftURL.path, rightPath: rightURL.path,
                maxBytes: UInt64(maxBytes))
        } catch let error as FfiIoError {
            switch error {
            case .Io(message: let message): throw HexLoadIOError(message: message)
            }
        }
        return HexComparison(ffi)
    }

    /// メモリ上の Data 同士を比較する(テスト・内部利用)。URL版と同じ上限保護を適用する
    public static func compare(left: Data, right: Data, maxBytes: Int = defaultByteCap) -> HexComparison {
        HexComparison(diffloupeHexCompareData(
            left: left, right: right, maxBytes: UInt64(maxBytes)))
    }

    /// バイト列をASCII表現に変換する。印字可能(0x20...0x7E)はそのまま、それ以外は "." にする
    public static func ascii(_ bytes: [UInt8]) -> String {
        diffloupeHexAscii(bytes: Data(bytes))
    }
}

// MARK: - FFI 変換

extension HexComparison {
    init(_ ffi: FfiHexComparison) {
        self.init(
            leftData: ffi.leftData,
            rightData: ffi.rightData,
            leftTotalSize: ffi.leftTotalSize,
            rightTotalSize: ffi.rightTotalSize,
            truncated: ffi.truncated
        )
    }
}
