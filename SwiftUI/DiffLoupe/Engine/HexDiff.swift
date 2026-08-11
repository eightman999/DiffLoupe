// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: バイナリ比較用の16バイト行Hex差分エンジン。ファイル全体をメモリに読み込まず、
// 上限バイト数(既定8MiB)までを安全に読み込み、行単位の差分情報とASCII表示を提供する(SPEC §2.6拡張)
// UIは表示行を必要時に生成するため、このエンジンが全行を事前に保持しないこと。

import Foundation

/// 16バイト行の1行分。そのオフセットにデータが存在しない側は bytes/ASCII を nil にする
/// (サイズ差による欠損を表す)。行内でバイト数が足りない(ファイル末尾)場合は不足分を欠損として扱う。
struct HexRow: Equatable, Sendable {
    let offset: Int
    let leftBytes: [UInt8]?
    let rightBytes: [UInt8]?
    let leftASCII: String?
    let rightASCII: String?
    /// 行内で左右が異なるバイトの行内インデックス(0..<bytesPerRow)。一致バイトは含まない。
    /// 片側にのみ存在するバイトは「差分」ではなく欠損として扱うため含まれない。
    let differingByteOffsets: [Int]
}

/// バイナリ比較の結果。読み込んだ上限分の Data を保持し、行は必要時に生成する
/// (8MiB上限でも行数は最大52万行超になるため、全行を事前生成してメモリに保持しない)。
struct HexComparison: Sendable {
    let leftData: Data
    let rightData: Data
    /// ファイルの総サイズ(読み込み上限前の実際のサイズ)。取得不能時は -1
    let leftTotalSize: Int64
    let rightTotalSize: Int64
    /// 片側でも総サイズが上限を超えた場合に true(超えた分は比較対象外)
    let truncated: Bool
    let bytesPerRow: Int = HexDiff.bytesPerRow

    /// 行数(サイズの大きい方のバイト数から算出)。空同士なら 0
    var rowCount: Int {
        let maxBytes = max(leftData.count, rightData.count)
        return (maxBytes + bytesPerRow - 1) / bytesPerRow
    }

    /// 左右が完全に一致しているか(総サイズ一致かつ読み込んだ範囲が一致)。空同士は true。
    /// truncated(読み込み上限で打ち切られた)場合は上限を超えた部分が不明のため、決して完全一致とは判定しない
    var isEqual: Bool {
        !truncated && leftTotalSize == rightTotalSize && leftData == rightData
    }

    /// 行インデックスに対応する行を返す。範囲外インデックスは安全な範囲へクランプする
    func row(at index: Int) -> HexRow {
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

enum HexDiff {

    /// 1行あたりのバイト数(16バイト行)
    static let bytesPerRow = 16

    /// 比較に読み込む既定の上限バイト数(8MiB)。これを超えるファイルは先頭分のみ比較し truncated で通知する
    static let defaultByteCap = 8 * 1024 * 1024

    /// ファイル同士を上限バイト数まで比較する。上限超過時は truncated が true になる。
    /// - Throws: ファイルの読み込みエラー(存在しない・アクセス不可など)
    static func compare(leftURL: URL, rightURL: URL, maxBytes: Int = defaultByteCap) throws -> HexComparison {
        let (leftData, leftTotal) = try readBounded(url: leftURL, maxBytes: maxBytes)
        let (rightData, rightTotal) = try readBounded(url: rightURL, maxBytes: maxBytes)
        return makeComparison(
            left: leftData, right: rightData,
            leftTotalSize: leftTotal, rightTotalSize: rightTotal,
            maxBytes: maxBytes)
    }

    /// メモリ上の Data 同士を比較する(テスト・内部利用)。URL版と同じ上限保護を適用する
    static func compare(left: Data, right: Data, maxBytes: Int = defaultByteCap) -> HexComparison {
        makeComparison(
            left: left, right: right,
            leftTotalSize: Int64(left.count), rightTotalSize: Int64(right.count),
            maxBytes: maxBytes)
    }

    /// バイト列をASCII表現に変換する。印字可能(0x20...0x7E)はそのまま、それ以外は "." にする
    static func ascii(_ bytes: [UInt8]) -> String {
        String(bytes.map { byte in
            (byte >= 0x20 && byte <= 0x7E) ? Character(UnicodeScalar(byte)) : "."
        })
    }

    // MARK: - 内部実装

    private static func makeComparison(
        left: Data, right: Data,
        leftTotalSize: Int64, rightTotalSize: Int64,
        maxBytes: Int
    ) -> HexComparison {
        let cap = max(maxBytes, 0)
        let leftBounded = left.count > cap ? Data(left.prefix(cap)) : left
        let rightBounded = right.count > cap ? Data(right.prefix(cap)) : right
        let leftTruncated = leftTotalSize >= 0
            ? leftTotalSize > Int64(cap)
            : (leftBounded.count == cap && cap > 0)
        let rightTruncated = rightTotalSize >= 0
            ? rightTotalSize > Int64(cap)
            : (rightBounded.count == cap && cap > 0)
        return HexComparison(
            leftData: leftBounded,
            rightData: rightBounded,
            leftTotalSize: leftTotalSize,
            rightTotalSize: rightTotalSize,
            truncated: leftTruncated || rightTruncated
        )
    }

    /// ファイルから上限バイト数までを安全に読み込む。総サイズは resourceValues から取得し、
    /// 読み込み上限の判定に使う。サイズ取得不能な特殊ファイルは上限バイト数を読み切る。
    private static func readBounded(url: URL, maxBytes: Int) throws -> (data: Data, totalSize: Int64) {
        let totalSize = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize)
            .flatMap { Int64($0) } ?? -1
        let readCount: Int
        if totalSize >= 0 {
            readCount = Int(min(totalSize, Int64(maxBytes)))
        } else {
            readCount = maxBytes
        }
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        let data = try handle.read(upToCount: readCount) ?? Data()
        return (data, totalSize)
    }
}
