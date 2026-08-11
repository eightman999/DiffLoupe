// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: バイナリファイル向けのHex差分ビュー。16バイト行ごとにオフセット・左右の16進ダンプ・
// ASCII表示を行い、異なるバイトを強調表示する。読み込みと差分生成はバックグラウンドで行う。
// 行数が多くてもメモリを圧迫しないよう LazyVStack + 行の遅延生成(HexComparison.row(at:))を使う。

import SwiftUI

struct HexComparisonView: View {
    let leftURL: URL
    let rightURL: URL

    private enum LoadState {
        case loading
        case loaded(HexComparison)
        case failed(String)
    }

    @State private var loadState: LoadState = .loading

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            content
        }
        .task(id: "\(leftURL.path)|\(rightURL.path)") {
            await load()
        }
    }

    // MARK: - ヘッダ

    @ViewBuilder
    private var header: some View {
        VStack(alignment: .leading, spacing: 2) {
            if case .loaded(let comparison) = loadState {
                HStack(spacing: 12) {
                    Text("左: \(sizeText(comparison.leftTotalSize))")
                    Text("右: \(sizeText(comparison.rightTotalSize))")
                    Text("比較済み: \(ByteCountFormatter.string(fromByteCount: Int64(comparison.leftData.count), countStyle: .file))")
                    if comparison.isEqual && !comparison.truncated {
                        Label("内容は一致しています", systemImage: "checkmark.circle")
                            .foregroundColor(.secondary)
                    }
                }
                .font(.caption2)
                .foregroundColor(.secondary)
                if comparison.truncated {
                    Label(
                        "上限(\(sizeText(Int64(HexDiff.defaultByteCap))))を超えるため、先頭分のみ表示しています",
                        systemImage: "doc.badge.ellipsis"
                    )
                    .font(.caption)
                    .foregroundColor(.orange)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    private func sizeText(_ size: Int64) -> String {
        size < 0 ? "不明" : ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    // MARK: - コンテンツ

    @ViewBuilder
    private var content: some View {
        switch loadState {
        case .loading:
            VStack(spacing: 10) {
                ProgressView()
                Text("バイナリ差分を計算中…").foregroundColor(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            VStack(spacing: 10) {
                Image(systemName: "exclamationmark.triangle")
                    .font(.system(size: 36))
                    .foregroundColor(.secondary.opacity(0.6))
                Text(message)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
            .padding()
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let comparison):
            loadedContent(comparison)
        }
    }

    private func loadedContent(_ comparison: HexComparison) -> some View {
        ScrollView([.vertical, .horizontal]) {
            LazyVStack(spacing: 0, pinnedViews: [.sectionHeaders]) {
                Section {
                    ForEach(0..<comparison.rowCount, id: \.self) { index in
                        HexRowView(
                            row: comparison.row(at: index),
                            bytesPerRow: comparison.bytesPerRow
                        )
                    }
                } header: {
                    HexColumnHeader(bytesPerRow: comparison.bytesPerRow)
                }
            }
        }
        .background(Color(nsColor: .textBackgroundColor))
    }

    private func load() async {
        loadState = .loading
        let result = await Task.detached(priority: .userInitiated) {
            Result {
                try HexDiff.compare(leftURL: leftURL, rightURL: rightURL)
            }
        }.value
        guard !Task.isCancelled else { return }
        switch result {
        case .success(let comparison):
            loadState = .loaded(comparison)
        case .failure(let error):
            loadState = .failed("読み込みエラー: \(error.localizedDescription)")
        }
    }
}

// MARK: - レイアウト定数

private enum HexLayout {
    /// オフセット列(8桁16進)
    static let offsetWidth: CGFloat = 76
    /// 16進バイト1個分(2桁)
    static let byteCellWidth: CGFloat = 15
    /// 16進バイト間の間隔
    static let byteSpacing: CGFloat = 2
    /// ASCII文字1個分
    static let asciiCellWidth: CGFloat = 8
    /// 列間の間隔
    static let columnSpacing: CGFloat = 12

    static var hexWidth: CGFloat { 16 * byteCellWidth + 15 * byteSpacing }
    static var asciiWidth: CGFloat { 16 * asciiCellWidth }
}

// MARK: - 列ヘッダ

private struct HexColumnHeader: View {
    let bytesPerRow: Int

    var body: some View {
        HStack(spacing: HexLayout.columnSpacing) {
            Text("オフセット")
                .frame(width: HexLayout.offsetWidth, alignment: .trailing)
            Text("左 (16進)")
                .frame(width: HexLayout.hexWidth, alignment: .leading)
            Text("右 (16進)")
                .frame(width: HexLayout.hexWidth, alignment: .leading)
            Text("左 ASCII")
                .frame(width: HexLayout.asciiWidth, alignment: .leading)
            Text("右 ASCII")
                .frame(width: HexLayout.asciiWidth, alignment: .leading)
        }
        .font(.system(size: 11, design: .monospaced))
        .foregroundColor(.secondary)
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(Color(nsColor: .controlBackgroundColor))
    }
}

// MARK: - 1行

private struct HexRowView: View {
    let row: HexRow
    let bytesPerRow: Int

    var body: some View {
        HStack(spacing: HexLayout.columnSpacing) {
            Text(String(format: "%08X", row.offset))
                .textSelection(.enabled)
                .frame(width: HexLayout.offsetWidth, alignment: .trailing)
                .foregroundColor(.secondary)
            HexByteColumn(
                bytes: row.leftBytes,
                bytesPerRow: bytesPerRow,
                differing: Set(row.differingByteOffsets)
            )
            HexByteColumn(
                bytes: row.rightBytes,
                bytesPerRow: bytesPerRow,
                differing: Set(row.differingByteOffsets)
            )
            ASCIIColumn(
                text: row.leftASCII,
                bytesPerRow: bytesPerRow,
                differing: Set(row.differingByteOffsets)
            )
            ASCIIColumn(
                text: row.rightASCII,
                bytesPerRow: bytesPerRow,
                differing: Set(row.differingByteOffsets)
            )
        }
        .font(.system(size: 11, design: .monospaced))
        .padding(.horizontal, 10)
        .padding(.vertical, 1)
    }
}

/// 16バイトの16進表示列。差分バイトは赤で強調し、データが無い位置は "··" で表す
private struct HexByteColumn: View {
    let bytes: [UInt8]?
    let bytesPerRow: Int
    let differing: Set<Int>

    var body: some View {
        HStack(spacing: HexLayout.byteSpacing) {
            ForEach(0..<bytesPerRow, id: \.self) { index in
                Text(hexText(index))
                    .frame(width: HexLayout.byteCellWidth, alignment: .center)
                    .foregroundColor(foregroundColor(index))
                    .background(backgroundColor(index))
            }
        }
        .frame(width: HexLayout.hexWidth, alignment: .leading)
    }

    private func hexText(_ index: Int) -> String {
        guard let bytes, index < bytes.count else { return "··" }
        return String(format: "%02X", bytes[index])
    }

    private func foregroundColor(_ index: Int) -> Color {
        guard let bytes, index < bytes.count else { return .secondary.opacity(0.5) }
        return differing.contains(index) ? .red : .primary
    }

    private func backgroundColor(_ index: Int) -> Color {
        guard let bytes, index < bytes.count, differing.contains(index) else { return .clear }
        return .red.opacity(0.20)
    }
}

/// 16バイト分のASCII表示列。差分バイト位置の文字は赤で強調し、データが無い位置は空白にする
private struct ASCIIColumn: View {
    let text: String?
    let bytesPerRow: Int
    let differing: Set<Int>

    var body: some View {
        HStack(spacing: 0) {
            ForEach(0..<bytesPerRow, id: \.self) { index in
                Text(charText(index))
                    .frame(width: HexLayout.asciiCellWidth, alignment: .center)
                    .foregroundColor(foregroundColor(index))
                    .background(backgroundColor(index))
            }
        }
        .frame(width: HexLayout.asciiWidth, alignment: .leading)
    }

    private func charText(_ index: Int) -> String {
        guard let text, index < text.count else { return " " }
        let start = text.index(text.startIndex, offsetBy: index)
        return String(text[start])
    }

    private func foregroundColor(_ index: Int) -> Color {
        guard let text, index < text.count else { return .secondary.opacity(0.4) }
        return differing.contains(index) ? .red : .primary
    }

    private func backgroundColor(_ index: Int) -> Color {
        guard let text, index < text.count, differing.contains(index) else { return .clear }
        return .red.opacity(0.20)
    }
}
