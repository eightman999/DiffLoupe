// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 画像ファイル向けの比較ビュー。読み込みと差分生成はバックグラウンドで行う

import SwiftUI

struct ImageComparisonView: View {
    let leftURL: URL
    let rightURL: URL

    private enum LoadState {
        case loading
        case loaded(ImageComparisonResult)
        case failed(String)
    }

    private enum ViewMode: String, CaseIterable, Identifiable {
        case sideBySide
        case overlay
        case blink
        case diff

        var id: String { rawValue }

        var label: String {
            switch self {
            case .sideBySide: return "左右"
            case .overlay: return "重ね合わせ"
            case .blink: return "ブリンク"
            case .diff: return "差分"
            }
        }
    }

    @State private var loadState: LoadState = .loading
    @State private var viewMode: ViewMode = .sideBySide
    @State private var overlayOpacity = 0.5
    @State private var threshold = Double(ImageDiffEngine.defaultThreshold)

    var body: some View {
        VStack(spacing: 0) {
            controls
            Divider()
            content
        }
        .task(id: taskID) {
            await load()
        }
    }

    private var taskID: String {
        "\(leftURL.path)|\(rightURL.path)|\(Int(threshold.rounded()))"
    }

    private var controls: some View {
        HStack(spacing: 10) {
            Picker("表示", selection: $viewMode) {
                ForEach(ViewMode.allCases) { mode in
                    Text(mode.label).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(width: 320)

            if viewMode == .overlay {
                Text("不透明度")
                    .foregroundColor(.secondary)
                Slider(value: $overlayOpacity, in: 0...1)
                    .frame(width: 140)
                Text("\(Int(overlayOpacity * 100))%")
                    .foregroundColor(.secondary)
                    .frame(width: 38, alignment: .trailing)
            }

            if viewMode == .diff {
                Text("閾値")
                    .foregroundColor(.secondary)
                Slider(value: $threshold, in: 0...255, step: 1)
                    .frame(width: 140)
                Text("\(Int(threshold.rounded()))")
                    .foregroundColor(.secondary)
                    .frame(width: 32, alignment: .trailing)
            }

            Spacer()
        }
        .font(.callout)
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    @ViewBuilder
    private var content: some View {
        switch loadState {
        case .loading:
            VStack(spacing: 10) {
                ProgressView()
                Text("画像差分を計算中…")
                    .foregroundColor(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let message):
            VStack(spacing: 10) {
                Image(systemName: "photo.badge.exclamationmark")
                    .font(.system(size: 36))
                    .foregroundColor(.secondary.opacity(0.6))
                Text(message)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
            .padding()
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .loaded(let result):
            loadedContent(result)
        }
    }

    private func loadedContent(_ result: ImageComparisonResult) -> some View {
        VStack(spacing: 0) {
            metadataBar(result)
            Divider()
            imageArea(result)
        }
    }

    private func metadataBar(_ result: ImageComparisonResult) -> some View {
        Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 4) {
            GridRow {
                Text("")
                Text("寸法")
                Text("ファイルサイズ")
                Text("形式")
            }
            .foregroundColor(.secondary)

            GridRow {
                Text("左").foregroundColor(.secondary)
                Text(result.leftMetadata.dimensionsText)
                Text(result.leftMetadata.fileSizeText)
                Text(result.leftMetadata.format)
            }

            GridRow {
                Text("右").foregroundColor(.secondary)
                Text(result.rightMetadata.dimensionsText)
                Text(result.rightMetadata.fileSizeText)
                Text(result.rightMetadata.format)
            }

            GridRow {
                Text("差分").foregroundColor(.secondary)
                Text(result.stats.comparedDimensionsText)
                Text("\(result.stats.diffPixelCount) px")
                Text(String(format: "%.2f%%", result.stats.diffPercentage))
            }
        }
        .font(.system(.caption, design: .monospaced))
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .trailing) {
            warningText(result)
                .font(.caption)
                .foregroundColor(.orange)
                .padding(.trailing, 10)
        }
    }

    @ViewBuilder
    private func warningText(_ result: ImageComparisonResult) -> some View {
        if result.stats.usedDownsampledComparison {
            Text("大きな画像のため差分計算は縮小画像で実行")
        } else if result.hasDifferentDimensions {
            Text("サイズ違い: 大きい方のキャンバスに正規化")
        }
    }

    @ViewBuilder
    private func imageArea(_ result: ImageComparisonResult) -> some View {
        switch viewMode {
        case .sideBySide:
            HStack(spacing: 1) {
                ImagePane(title: "左", image: result.leftImage)
                Divider()
                ImagePane(title: "右", image: result.rightImage)
            }
        case .overlay:
            OverlayPane(leftImage: result.leftImage, rightImage: result.rightImage, opacity: overlayOpacity)
        case .blink:
            BlinkPane(leftImage: result.leftImage, rightImage: result.rightImage)
        case .diff:
            ImagePane(title: "差分ハイライト", image: result.diffImage)
        }
    }

    private func load() async {
        loadState = .loading
        let thresholdValue = Int(threshold.rounded())
        let result = await Task.detached(priority: .userInitiated) {
            Result {
                try ImageDiffEngine.compare(leftURL: leftURL, rightURL: rightURL, threshold: thresholdValue)
            }
        }.value

        guard !Task.isCancelled else { return }
        switch result {
        case .success(let comparison):
            loadState = .loaded(comparison)
        case .failure(let error):
            loadState = .failed(error.localizedDescription)
        }
    }
}

private struct ImagePane: View {
    let title: String
    let image: CGImage

    var body: some View {
        VStack(spacing: 0) {
            Text(title)
                .font(.caption)
                .foregroundColor(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
            Divider()
            ScrollView([.horizontal, .vertical]) {
                Image(decorative: image, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .padding(10)
            }
            .background(Color(nsColor: .textBackgroundColor))
        }
    }
}

private struct OverlayPane: View {
    let leftImage: CGImage
    let rightImage: CGImage
    let opacity: Double

    var body: some View {
        ScrollView([.horizontal, .vertical]) {
            ZStack {
                Image(decorative: leftImage, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                Image(decorative: rightImage, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .opacity(opacity)
            }
            .padding(10)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .background(Color(nsColor: .textBackgroundColor))
    }
}

private struct BlinkPane: View {
    let leftImage: CGImage
    let rightImage: CGImage

    var body: some View {
        TimelineView(.periodic(from: .now, by: 0.65)) { timeline in
            let showRight = Int(timeline.date.timeIntervalSinceReferenceDate / 0.65).isMultiple(of: 2)
            ImagePane(title: showRight ? "右" : "左", image: showRight ? rightImage : leftImage)
        }
    }
}
