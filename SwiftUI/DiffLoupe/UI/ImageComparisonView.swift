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
        case slice

        var id: String { rawValue }

        var label: String {
            switch self {
            case .sideBySide: return "左右"
            case .overlay: return "重ね合わせ"
            case .blink: return "ブリンク"
            case .diff: return "差分"
            case .slice: return "スライス"
            }
        }
    }

    @State private var loadState: LoadState = .loading
    @State private var viewMode: ViewMode = .sideBySide
    @State private var overlayOpacity = 0.5
    @State private var threshold = Double(ImageDiffEngine.defaultThreshold)
    // スライス比較の境界位置（0.0: 左端 / 1.0: 右端）
    @State private var slicePosition = 0.5
    // ズーム・パン状態。左右ペインとoverlay/blink/diff/sliceペインで共有して位置合わせに使う
    @State private var viewport = ImageViewport()
    // 閾値スライダーの変更をデバウンスするための再読み込み回数。操作のたびに再計算しない
    @State private var reloadNonce = 0
    @State private var thresholdDebounceTask: Task<Void, Never>?

    var body: some View {
        VStack(spacing: 0) {
            controls
            Divider()
            content
        }
        .task(id: taskID) {
            await load()
        }
        .onChange(of: threshold) { _ in
            scheduleThresholdReload()
        }
        .onChange(of: viewMode) { _ in
            // モード切替時はパン位置をリセットして共有ビューポートを安全にする(ズームは維持)
            viewport.offset = .zero
        }
    }

    private var taskID: String {
        "\(leftURL.path)|\(rightURL.path)|\(reloadNonce)"
    }

    private var controls: some View {
        // 狭いウィンドウでも操作が途切れないよう、全コントロールを横スクロール可能にする
        ScrollView(.horizontal, showsIndicators: false) {
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

                if viewMode == .slice {
                    Text("境界")
                        .foregroundColor(.secondary)
                    Slider(value: $slicePosition, in: 0...1)
                        .frame(width: 140)
                    Text("\(Int(slicePosition * 100))%")
                        .foregroundColor(.secondary)
                        .frame(width: 38, alignment: .trailing)
                }

                zoomControls

                Spacer()
            }
            .font(.callout)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
        }
    }

    /// ズーム操作(縮小/拡大ボタン・スライダー・リセット)。0.25x〜4.0xの範囲に制限
    private var zoomControls: some View {
        HStack(spacing: 6) {
            Divider()
                .frame(height: 14)

            Text("ズーム")
                .foregroundColor(.secondary)

            Button {
                viewport.scale = ImageViewport.clampedScale(viewport.scale / 1.25)
            } label: {
                Image(systemName: "minus.magnifyingglass")
            }
            .help("ズームアウト")
            .disabled(viewport.scale <= ImageViewport.minScale)

            Slider(value: zoomScale, in: ImageViewport.minScale...ImageViewport.maxScale)
                .frame(width: 140)

            Button {
                viewport.scale = ImageViewport.clampedScale(viewport.scale * 1.25)
            } label: {
                Image(systemName: "plus.magnifyingglass")
            }
            .help("ズームイン")
            .disabled(viewport.scale >= ImageViewport.maxScale)

            Text("\(Int((viewport.scale * 100).rounded()))%")
                .foregroundColor(.secondary)
                .monospacedDigit()
                .frame(width: 46, alignment: .trailing)

            Button {
                viewport = ImageViewport()
            } label: {
                Image(systemName: "arrow.counterclockwise")
            }
            .help("フィット表示に戻す")
            .disabled(viewport == ImageViewport())
        }
    }

    private var zoomScale: Binding<Double> {
        Binding(
            get: { Double(viewport.scale) },
            set: { viewport.scale = ImageViewport.clampedScale(CGFloat($0)) }
        )
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
        VStack(alignment: .leading, spacing: 4) {
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

            // 警告はGridの下に置く(右端列との重なりを避ける)
            warningText(result)
                .font(.caption)
                .foregroundColor(.orange)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .frame(maxWidth: .infinity, alignment: .leading)
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
            // 異なる寸法の画像でも共有ズーム/パンが整合するよう、両ペインで共通キャンバスサイズを使う
            let canvas = comparisonCanvasSize(left: result.leftImage, right: result.rightImage)
            HStack(spacing: 1) {
                ImagePane(title: "左", image: result.leftImage, canvasSize: canvas, viewport: $viewport)
                Divider()
                ImagePane(title: "右", image: result.rightImage, canvasSize: canvas, viewport: $viewport)
            }
        case .overlay:
            OverlayPane(
                leftImage: result.leftImage,
                rightImage: result.rightImage,
                opacity: overlayOpacity,
                viewport: $viewport
            )
        case .blink:
            BlinkPane(
                leftImage: result.leftImage,
                rightImage: result.rightImage,
                viewport: $viewport
            )
        case .diff:
            ImagePane(
                title: "差分ハイライト",
                image: result.diffImage,
                canvasSize: cgImageSize(result.diffImage),
                viewport: $viewport
            )
        case .slice:
            SlicePane(
                leftImage: result.leftImage,
                rightImage: result.rightImage,
                position: $slicePosition,
                viewport: $viewport
            )
        }
    }

    /// 閾値スライダーの変更をデバウンスして再読み込みを一度だけ行う。
    /// 操作中のステップごとに画像再デコードを始めず、既存の結果表示は確定まで保持する。
    /// 確定後に reloadNonce を進めて .task(id:) を再起動するため、古い結果のキャンセル挙動は維持される。
    private func scheduleThresholdReload() {
        thresholdDebounceTask?.cancel()
        let task = Task { @MainActor in
            try? await Task.sleep(nanoseconds: 400_000_000)
            guard !Task.isCancelled else { return }
            reloadNonce += 1
        }
        thresholdDebounceTask = task
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

// MARK: - 共有ビューポート

// ズーム(0.25x〜4.0x)とパンを全ペインで共有するビューポート状態。
// ズーム基準は「画像をビューにアスペクト比維持でフィットした状態 = 1.0」。
private struct ImageViewport: Equatable {
    static let minScale: CGFloat = 0.25
    static let maxScale: CGFloat = 4.0

    var scale: CGFloat = 1.0
    var offset: CGSize = .zero

    static func clampedScale(_ scale: CGFloat) -> CGFloat {
        min(max(scale, minScale), maxScale)
    }

    /// 画像をビューに収めたときの表示サイズ(アスペクト比維持)
    static func fittedSize(for imageSize: CGSize, in viewSize: CGSize) -> CGSize {
        guard imageSize.width > 0, imageSize.height > 0,
              viewSize.width > 0, viewSize.height > 0 else { return .zero }
        let ratio = min(viewSize.width / imageSize.width, viewSize.height / imageSize.height)
        return CGSize(width: imageSize.width * ratio, height: imageSize.height * ratio)
    }

    /// ズーム適用後の表示サイズ
    static func scaled(_ size: CGSize, by scale: CGFloat) -> CGSize {
        CGSize(width: size.width * scale, height: size.height * scale)
    }

    /// パンの許容範囲。画像が画面外に完全に消えない最大オフセット
    static func panLimits(displayedSize: CGSize, viewSize: CGSize) -> CGSize {
        CGSize(
            width: max(0, (displayedSize.width - viewSize.width) / 2),
            height: max(0, (displayedSize.height - viewSize.height) / 2)
        )
    }

    static func clampOffset(_ offset: CGSize, limits: CGSize) -> CGSize {
        CGSize(
            width: min(max(offset.width, -limits.width), limits.width),
            height: min(max(offset.height, -limits.height), limits.height)
        )
    }
}

// 共有ビューポートに従い、ズーム(ピンチ)とパン(ドラッグ)を提供するキャンバス。
// content はフィット×ズーム後の表示サイズを受け取り、その座標系で描画する。
// パンは画像が画面外に完全に消えない範囲にクランプされる。
private struct ZoomableCanvas<Content: View>: View {
    @Binding var viewport: ImageViewport
    /// ズーム基準となる画像サイズ(フィット計算に使用)
    let imageSize: CGSize
    @ViewBuilder var content: (CGSize) -> Content

    @State private var gestureScale: CGFloat = 1
    @State private var gestureOffset: CGSize = .zero
    @State private var canvasSize: CGSize = .zero

    var body: some View {
        GeometryReader { geo in
            let fitted = ImageViewport.fittedSize(for: imageSize, in: geo.size)
            let liveScale = ImageViewport.clampedScale(viewport.scale * gestureScale)
            let displayed = ImageViewport.scaled(fitted, by: liveScale)
            let limits = ImageViewport.panLimits(displayedSize: displayed, viewSize: geo.size)
            let liveOffset = ImageViewport.clampOffset(
                CGSize(
                    width: viewport.offset.width + gestureOffset.width,
                    height: viewport.offset.height + gestureOffset.height
                ),
                limits: limits
            )

            ZStack {
                content(displayed)
                    .frame(width: displayed.width, height: displayed.height)
                    .position(x: geo.size.width / 2 + liveOffset.width,
                              y: geo.size.height / 2 + liveOffset.height)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .clipped()
            .contentShape(Rectangle())
            .gesture(panGesture)
            .simultaneousGesture(magnificationGesture)
            .background(Color(nsColor: .textBackgroundColor))
            .onAppear { canvasSize = geo.size }
            .onChange(of: geo.size) { newSize in canvasSize = newSize }
            .onChange(of: viewport.scale) { _ in normalizeStoredOffset() }
        }
    }

    private var panGesture: some Gesture {
        DragGesture()
            .onChanged { value in
                gestureOffset = value.translation
            }
            .onEnded { value in
                let displayed = ImageViewport.scaled(
                    ImageViewport.fittedSize(for: imageSize, in: canvasSize),
                    by: viewport.scale
                )
                let limits = ImageViewport.panLimits(displayedSize: displayed, viewSize: canvasSize)
                viewport.offset = ImageViewport.clampOffset(
                    CGSize(
                        width: viewport.offset.width + value.translation.width,
                        height: viewport.offset.height + value.translation.height
                    ),
                    limits: limits
                )
                gestureOffset = .zero
            }
    }

    private var magnificationGesture: some Gesture {
        MagnificationGesture()
            .onChanged { value in
                gestureScale = value
            }
            .onEnded { value in
                viewport.scale = ImageViewport.clampedScale(viewport.scale * value)
                gestureScale = 1
            }
    }

    /// ズーム変更後に保存済みオフセットを新しいパン範囲へ再クランプする
    private func normalizeStoredOffset() {
        let displayed = ImageViewport.scaled(
            ImageViewport.fittedSize(for: imageSize, in: canvasSize),
            by: viewport.scale
        )
        let limits = ImageViewport.panLimits(displayedSize: displayed, viewSize: canvasSize)
        viewport.offset = ImageViewport.clampOffset(viewport.offset, limits: limits)
    }
}

// MARK: - 各比較ペイン

private struct ImagePane: View {
    let title: String
    let image: CGImage
    /// ズーム・パン基準となる共通キャンバスサイズ。左右の画像寸法が異なる場合でも
    /// 共有ビューポートのスケール/オフセットが両ペインで整合するようにする。
    let canvasSize: CGSize
    @Binding var viewport: ImageViewport

    var body: some View {
        VStack(spacing: 0) {
            Text(title)
                .font(.caption)
                .foregroundColor(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
            Divider()
            ZoomableCanvas(viewport: $viewport, imageSize: canvasSize) { displayed in
                Image(decorative: image, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .frame(width: displayed.width, height: displayed.height)
            }
        }
    }
}

private struct OverlayPane: View {
    let leftImage: CGImage
    let rightImage: CGImage
    let opacity: Double
    @Binding var viewport: ImageViewport

    var body: some View {
        ZoomableCanvas(
            viewport: $viewport,
            imageSize: comparisonCanvasSize(left: leftImage, right: rightImage)
        ) { displayed in
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
            .frame(width: displayed.width, height: displayed.height)
        }
    }
}

private struct BlinkPane: View {
    let leftImage: CGImage
    let rightImage: CGImage
    @Binding var viewport: ImageViewport

    var body: some View {
        // 左右の寸法が異なっても切り替え時にサイズが脈動しないよう固定の共通キャンバスを使う
        let canvas = comparisonCanvasSize(left: leftImage, right: rightImage)
        TimelineView(.periodic(from: .now, by: 0.65)) { timeline in
            let showRight = Int(timeline.date.timeIntervalSinceReferenceDate / 0.65).isMultiple(of: 2)
            ImagePane(
                title: showRight ? "右" : "左",
                image: showRight ? rightImage : leftImage,
                canvasSize: canvas,
                viewport: $viewport
            )
        }
    }
}

// スライス（ワイプ）比較: 両画像を同位置に重ね、縦の境界線で左右を切り替える
private struct SlicePane: View {
    let leftImage: CGImage
    let rightImage: CGImage
    @Binding var position: Double
    @Binding var viewport: ImageViewport

    /// ドラッグ開始時の境界位置。ジェスチャ開始時に記録する
    @State private var dividerDragStart = 0.5

    var body: some View {
        ZoomableCanvas(
            viewport: $viewport,
            imageSize: comparisonCanvasSize(left: leftImage, right: rightImage)
        ) { displayed in
            let dividerX = displayed.width * CGFloat(position)

            ZStack {
                // 右画像: 境界より右側のみ表示
                Image(decorative: rightImage, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .frame(width: displayed.width, height: displayed.height)
                    .mask(
                        Color.black
                            .frame(width: displayed.width - dividerX, height: displayed.height)
                            .frame(width: displayed.width, height: displayed.height, alignment: .trailing)
                    )

                // 左画像: 境界より左側のみ表示
                Image(decorative: leftImage, scale: 1)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .frame(width: displayed.width, height: displayed.height)
                    .mask(
                        Color.black
                            .frame(width: dividerX, height: displayed.height)
                            .frame(width: displayed.width, height: displayed.height, alignment: .leading)
                    )

                // 境界ライン
                Rectangle()
                    .fill(Color.white.opacity(0.9))
                    .frame(width: 2, height: displayed.height)
                    .position(x: dividerX, y: displayed.height / 2)
                    .shadow(color: .black.opacity(0.35), radius: 1)

                // ドラッグハンドル(幅広の当たり判定)。
                // 子ビューのジェスチャが親キャンバスのパンより優先されるため、
                // ハンドル上でのドラッグは境界移動、それ以外はパンになる
                dividerHandle(x: dividerX, in: displayed)
            }
            .frame(width: displayed.width, height: displayed.height)
        }
    }

    private func dividerHandle(x: CGFloat, in size: CGSize) -> some View {
        let handleWidth: CGFloat = 24

        return ZStack {
            Rectangle()
                .fill(Color.clear)
                .frame(width: handleWidth, height: size.height)
            Circle()
                .fill(Color.white)
                .overlay(
                    Circle().stroke(Color.black.opacity(0.5), lineWidth: 1)
                )
                .frame(width: 16, height: 16)
                .overlay(
                    Image(systemName: "arrow.left.and.right")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundColor(.black.opacity(0.6))
                )
        }
        .frame(width: handleWidth, height: size.height)
        .position(x: x, y: size.height / 2)
        .contentShape(Rectangle())
        .gesture(
            DragGesture(minimumDistance: 0)
                .onChanged { value in
                    // 最初のイベント(location == startLocation)で基準位置を記録する
                    if value.location == value.startLocation {
                        dividerDragStart = position
                    }
                    let delta = value.location.x - value.startLocation.x
                    let normalized = dividerDragStart + Double(delta / size.width)
                    position = min(max(0, normalized), 1)
                }
        )
    }
}

// MARK: - ヘルパー

private func cgImageSize(_ image: CGImage) -> CGSize {
    CGSize(width: image.width, height: image.height)
}

/// エンジンの差分キャンバスと同じ「大きい方に正規化」したサイズ(overlay/slice用)
private func comparisonCanvasSize(left: CGImage, right: CGImage) -> CGSize {
    CGSize(width: max(left.width, right.width), height: max(left.height, right.height))
}
