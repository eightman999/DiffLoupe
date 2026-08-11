// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 選択ファイルの横並び行差分ビュー。差分計算はバックグラウンドで行いキャンセルに追随する

import SwiftUI

struct DiffDetailView: View {
    let entry: ComparedEntry?
    let contentComparison: ContentComparison

    private enum DetailState: Sendable {
        case empty
        case loading
        case message(String, systemImage: String)
        case diff(
            [DiffRow],
            highlights: [Int: InlineHighlight],
            leftEncoding: String,
            rightEncoding: String,
            structuredInfo: StructuredInfo?
        )
        case image(URL, URL)
        case hex(URL, URL)
    }

    @State private var state: DetailState = .empty

    /// 差分行ナビゲーションの現在位置(差分行リスト内のインデックス)。nil = 未選択
    @State private var diffIndex: Int?
    /// 行内差分ビューの検索状態
    @State private var searchText = ""
    @State private var searchCaseSensitive = false
    @State private var currentSearchHit: Int?
    /// コンテキスト折りたたみの展開状態(グループIDの集合)。既定は折りたたみ
    @State private var expandedGroups: Set<Int> = []
    /// 折りたたみを無効にして全行表示するトグル
    @State private var showAllContextRows = false
    /// ScrollViewReader へスクロールを依頼する表示アイテムID
    @State private var scrollTarget: Int?

    /// 再読み込みトリガー。ファイルパスと内容比較設定の両方が変わるまで再計算しない
    /// (設定変更後の再表示はsignatureの変化で検出する)。
    private struct LoadKey: Equatable {
        let path: String?
        let comparisonSignature: String
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            Divider()
            if case .diff = state {
                toolbar
                Divider()
            }
            content
        }
        .task(id: LoadKey(path: entry?.relativePath, comparisonSignature: contentComparison.settingsSignature)) {
            await load()
        }
        .onChange(of: searchText) { _ in
            currentSearchHit = nil
            expandFoldGroupsForSearch()
        }
        .onChange(of: searchCaseSensitive) { _ in
            currentSearchHit = nil
            expandFoldGroupsForSearch()
        }
    }

    // MARK: - 差分行ナビゲーション・検索・折りたたみ

    private var diffRows: [DiffRow] {
        if case .diff(let rows, _, _, _, _) = state { return rows }
        return []
    }

    private var differenceRowIDs: [Int] {
        DiffDisplay.differenceRowIDs(in: diffRows)
    }

    private var currentDifferenceRowID: Int? {
        guard let diffIndex, differenceRowIDs.indices.contains(diffIndex) else { return nil }
        return differenceRowIDs[diffIndex]
    }

    private var searchHitRowIDs: [Int] {
        DiffDisplay.searchHitRowIDs(in: diffRows, query: searchText, caseSensitive: searchCaseSensitive)
    }

    private var currentSearchHitRowID: Int? {
        guard let currentSearchHit, searchHitRowIDs.indices.contains(currentSearchHit) else { return nil }
        return searchHitRowIDs[currentSearchHit]
    }

    private var currentSearchPosition: String {
        guard let currentSearchHit, searchHitRowIDs.indices.contains(currentSearchHit) else { return "—" }
        return "\(currentSearchHit + 1)"
    }

    private func jumpToDifference(_ offset: Int) {
        let ids = differenceRowIDs
        guard !ids.isEmpty else { return }
        let current = diffIndex ?? (offset > 0 ? -1 : ids.count)
        let next = current + offset
        guard next >= 0, next < ids.count else { return }
        diffIndex = next
        requestScrollToRow(ids[next])
    }

    private func jumpToSearchHit(_ offset: Int) {
        let ids = searchHitRowIDs
        guard !ids.isEmpty else { return }
        let current = currentSearchHit ?? (offset > 0 ? -1 : ids.count)
        let next = current + offset
        guard next >= 0, next < ids.count else { return }
        currentSearchHit = next
        requestScrollToRow(ids[next])
    }

    /// 行IDでスクロール位置を制御する。対象行が折りたたみグループ内なら先に展開する
    private func requestScrollToRow(_ rowID: Int) {
        if let group = DiffDisplay.foldGroups(in: diffRows).first(where: { $0.rowIDs.contains(rowID) }) {
            expandedGroups.insert(group.id)
        }
        let items = DiffDisplay.displayItems(
            rows: diffRows,
            expandedGroupIDs: expandedGroups,
            showAllContextRows: showAllContextRows
        )
        if let target = items.first(where: { item in
            if case .row(let row) = item, row.id == rowID { return true }
            return false
        })?.id {
            scrollTarget = target
        }
    }

    private func resetNavigationState() {
        diffIndex = nil
        currentSearchHit = nil
        searchText = ""
        expandedGroups = []
        showAllContextRows = false
        scrollTarget = nil
    }

    /// 現在の検索ヒット行を含む折りたたみグループを展開する。
    /// クエリ空時(検索解除)は何もしないので、既存の折りたたみ状態を壊さない。
    private func expandFoldGroupsForSearch() {
        let hits = searchHitRowIDs
        guard !hits.isEmpty else { return }
        expandedGroups.formUnion(DiffDisplay.expandedGroupIDs(containingRowIDs: hits, in: diffRows))
    }

    // MARK: - ツールバー

    @ViewBuilder
    private var toolbar: some View {
        HStack(spacing: 8) {
            Button {
                jumpToDifference(-1)
            } label: {
                Image(systemName: "chevron.up")
            }
            .help("前の差分")
            .keyboardShortcut(.upArrow, modifiers: .command)
            .disabled(differenceRowIDs.isEmpty || (diffIndex.map { $0 == 0 } ?? false))

            Button {
                jumpToDifference(1)
            } label: {
                Image(systemName: "chevron.down")
            }
            .help("次の差分")
            .keyboardShortcut(.downArrow, modifiers: .command)
            .disabled(differenceRowIDs.isEmpty || (diffIndex.map { $0 == differenceRowIDs.count - 1 } ?? false))

            if differenceRowIDs.count > 0 {
                Text("\(diffIndex.map { "\($0 + 1)" } ?? "—")/\(differenceRowIDs.count)")
                    .font(.caption)
                    .monospacedDigit()
                    .foregroundColor(.secondary)
            }

            Divider()
                .frame(height: 14)

            TextField("検索", text: $searchText)
                .textFieldStyle(.roundedBorder)
                .frame(width: 160)

            Button {
                jumpToSearchHit(-1)
            } label: {
                Image(systemName: "chevron.up")
            }
            .help("前の一致")
            .keyboardShortcut("g", modifiers: [.command, .shift])
            .disabled(searchHitRowIDs.isEmpty)

            Button {
                jumpToSearchHit(1)
            } label: {
                Image(systemName: "chevron.down")
            }
            .help("次の一致")
            .keyboardShortcut("g", modifiers: .command)
            .disabled(searchHitRowIDs.isEmpty)

        if !searchHitRowIDs.isEmpty {
            Text("\(currentSearchPosition)/\(searchHitRowIDs.count)")
                    .font(.caption)
                    .monospacedDigit()
                    .foregroundColor(.secondary)
            }

            Button {
                searchCaseSensitive.toggle()
            } label: {
                Label(
                    searchCaseSensitive ? "Aa" : "aA",
                    systemImage: searchCaseSensitive ? "textformat.alt" : "textformat"
                )
            }
            .help("大文字と小文字を区別する")
            .buttonStyle(.borderless)

            Spacer()

            Toggle("全行表示", isOn: $showAllContextRows)
                .toggleStyle(.checkbox)
                .help("コンテキスト行の折りたたみを無効にする")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var header: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                if let entry {
                    Image(systemName: entry.isDirectory ? "folder.fill" : "doc.text")
                        .foregroundColor(.secondary)
                    Text(entry.relativePath)
                        .font(.system(.callout, design: .monospaced))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    StatusBadge(status: entry.status, isDirectory: entry.isDirectory)
                } else {
                    Text("ファイル差分")
                        .foregroundColor(.secondary)
                    Spacer()
                }
            }
            if let entry, !entry.isDirectory {
                MetadataBar(entry: entry)
            }
            if case .diff(_, _, let leftEncoding, let rightEncoding, let structuredInfo) = state {
                HStack(spacing: 8) {
                    Text("左: \(leftEncoding)  /  右: \(rightEncoding)")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                    if let structuredInfo {
                        StructuredBadge(info: structuredInfo)
                    }
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
    }

    @ViewBuilder
    private var content: some View {
        switch state {
        case .empty:
            placeholder("ツリーからファイルを選択してください", systemImage: "cursorarrow.rays")
        case .loading:
            VStack(spacing: 10) {
                ProgressView()
                Text("差分を計算中…").foregroundColor(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .message(let text, let systemImage):
            placeholder(text, systemImage: systemImage)
        case .diff(let rows, let highlights, _, _, _):
            DiffRowsView(
                rows: rows,
                highlights: highlights,
                expandedGroups: $expandedGroups,
                showAllContextRows: $showAllContextRows,
                currentDifferenceID: currentDifferenceRowID,
                searchHitRowIDs: Set(searchHitRowIDs),
                currentSearchHitID: currentSearchHitRowID,
                onExpandGroup: { groupID in
                    expandedGroups.insert(groupID)
                },
                scrollTarget: $scrollTarget
            )
        case .image(let leftURL, let rightURL):
            ImageComparisonView(leftURL: leftURL, rightURL: rightURL)
        case .hex(let leftURL, let rightURL):
            HexComparisonView(leftURL: leftURL, rightURL: rightURL)
        }
    }

    private func placeholder(_ text: String, systemImage: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: systemImage)
                .font(.system(size: 36))
                .foregroundColor(.secondary.opacity(0.6))
            Text(text)
                .foregroundColor(.secondary)
                .multilineTextAlignment(.center)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - 差分計算

    private func load() async {
        resetNavigationState()
        guard let entry else {
            state = .empty
            return
        }
        if entry.isDirectory {
            state = .message("フォルダです。配下のファイルを選択してください", systemImage: "folder")
            return
        }
        switch entry.status {
        case .leftOnly:
            state = .message("左フォルダにのみ存在します", systemImage: "arrow.left.circle")
            return
        case .rightOnly:
            state = .message("右フォルダにのみ存在します", systemImage: "arrow.right.circle")
            return
        case .error:
            state = .message("読み込みエラー: \(entry.errorMessage ?? "不明")", systemImage: "exclamationmark.triangle")
            return
        case .different:
            break
        case .identical:
            break
        }
        guard let leftURL = entry.leftURL, let rightURL = entry.rightURL else {
            state = .message("比較対象がありません", systemImage: "questionmark.circle")
            return
        }

        if ImageFileDetector.isSupportedImageFile(leftURL),
           ImageFileDetector.isSupportedImageFile(rightURL) {
            state = .image(leftURL, rightURL)
            return
        }

        if entry.status == .identical {
            state = .message("内容は一致しています", systemImage: "checkmark.circle")
            return
        }

        state = .loading
        let computed = await Self.computeDiff(
            leftURL: leftURL, rightURL: rightURL, contentComparison: contentComparison)
        if !Task.isCancelled {
            state = computed
        }
    }

    private static func computeDiff(
        leftURL: URL,
        rightURL: URL,
        contentComparison: ContentComparison
    ) async -> DetailState {
        let task = Task.detached(priority: .userInitiated) { () -> DetailState in
            do {
                let left = try TextFileLoader.load(url: leftURL)
                let right = try TextFileLoader.load(url: rightURL)
                switch (left, right) {
                case (.binary, _), (_, .binary):
                    return .hex(leftURL, rightURL)
                case (.tooLarge, _), (_, .tooLarge):
                    return .message("ファイルが大きすぎるため差分表示できません(上限32MiB)", systemImage: "doc.badge.ellipsis")
                case (.text(let leftText, let leftEncoding), .text(let rightText, let rightEncoding)):
                    // 構造化モード: 両側とも正規化に成功した場合のみ正規化テキストを比較・表示する。
                    // 片側でも失敗したら元テキストで比較し、警告バッジでフォールバック理由を示す。
                    if contentComparison.structuredMode != .disabled {
                        let mode = contentComparison.structuredMode
                        let leftResult = StructuredNormalizer.normalize(
                            Data(leftText.utf8), url: leftURL, mode: mode)
                        let rightResult = StructuredNormalizer.normalize(
                            Data(rightText.utf8), url: rightURL, mode: mode)
                        if leftResult.didNormalize, rightResult.didNormalize,
                           let normalizedLeft = leftResult.normalizedText,
                           let normalizedRight = rightResult.normalizedText {
                            if let rows = LineDiff.rows(
                                leftText: normalizedLeft,
                                rightText: normalizedRight,
                                contentComparison: contentComparison
                            ) {
                                let highlights = InlineDiff.highlightsForRows(
                                    rows, caseInsensitive: contentComparison.ignoreCase)
                                return .diff(
                                    rows,
                                    highlights: highlights,
                                    leftEncoding: leftEncoding,
                                    rightEncoding: rightEncoding,
                                    structuredInfo: StructuredInfo(
                                        format: leftResult.format ?? rightResult.format,
                                        didNormalize: true,
                                        leftNotice: nil,
                                        rightNotice: nil
                                    )
                                )
                            }
                            return .message("行数が多すぎるため差分計算を省略しました(上限 計\(LineDiff.maxTotalLines)行)", systemImage: "doc.badge.ellipsis")
                        }
                        // フォールバック: 元テキストの比較 + 警告
                        let fallbackInfo = StructuredInfo(
                            format: leftResult.format ?? rightResult.format,
                            didNormalize: false,
                            leftNotice: leftResult.notice,
                            rightNotice: rightResult.notice
                        )
                        if let rows = LineDiff.rows(
                            leftText: leftText, rightText: rightText, contentComparison: contentComparison) {
                            let highlights = InlineDiff.highlightsForRows(
                                rows, caseInsensitive: contentComparison.ignoreCase)
                            return .diff(
                                rows,
                                highlights: highlights,
                                leftEncoding: leftEncoding,
                                rightEncoding: rightEncoding,
                                structuredInfo: fallbackInfo
                            )
                        }
                        return .message("行数が多すぎるため差分計算を省略しました(上限 計\(LineDiff.maxTotalLines)行)", systemImage: "doc.badge.ellipsis")
                    }
                    if let rows = LineDiff.rows(
                        leftText: leftText, rightText: rightText, contentComparison: contentComparison) {
                        let highlights = InlineDiff.highlightsForRows(
                            rows, caseInsensitive: contentComparison.ignoreCase)
                        return .diff(rows, highlights: highlights, leftEncoding: leftEncoding, rightEncoding: rightEncoding, structuredInfo: nil)
                    }
                    return .message("行数が多すぎるため差分計算を省略しました(上限 計\(LineDiff.maxTotalLines)行)", systemImage: "doc.badge.ellipsis")
                }
            } catch {
                return .message("読み込みエラー: \(error.localizedDescription)", systemImage: "exclamationmark.triangle")
            }
        }
        return await task.value
    }
}

/// 構造化比較の表示情報。didNormalize == false のときはフォールバックを警告表示する。
struct StructuredInfo: Sendable, Equatable {
    let format: StructuredFormat?
    let didNormalize: Bool
    let leftNotice: String?
    let rightNotice: String?

    var fallbackReasonText: String {
        var parts: [String] = []
        if let leftNotice, !leftNotice.isEmpty { parts.append("左: \(leftNotice)") }
        if let rightNotice, !rightNotice.isEmpty { parts.append("右: \(rightNotice)") }
        if parts.isEmpty { return "" }
        return " (" + parts.joined(separator: " / ") + ")"
    }
}

/// 構造化比較の状態を示すコンパクトなバッジ。成功時は緑、フォールバック時はオレンジ警告。
private struct StructuredBadge: View {
    let info: StructuredInfo

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: info.didNormalize ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
            Text(badgeText)
                .lineLimit(1)
                .truncationMode(.middle)
        }
        .font(.caption2)
        .foregroundColor(info.didNormalize ? .green : .orange)
        .help(helpText)
    }

    private var badgeText: String {
        if info.didNormalize {
            return "\(info.format?.displayName ?? "構造化")正規化で比較"
        }
        return "構造化正規化に失敗" + info.fallbackReasonText
    }

    private var helpText: String {
        if info.didNormalize {
            return "\(info.format?.displayName ?? "")の表記揺れを正規化して比較しています"
        }
        return "片側または両側のファイルを構造化パースできなかったため、元のテキストで比較しています。\(info.fallbackReasonText)"
    }
}

/// 選択ファイルのリネーム/移動候補と左右メタデータ(サイズ・変更日時・POSIXパーミッション)の表示。
private struct MetadataBar: View {
    let entry: ComparedEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            if let rename = entry.renameCandidate {
                RenameCandidateBar(candidate: rename)
            }
            HStack(spacing: 12) {
                MetadataItem(label: "サイズ", leftText: Self.sizeText(entry.leftSize), rightText: Self.sizeText(entry.rightSize))
                MetadataItem(label: "変更日時", leftText: Self.dateText(entry.leftModificationDate), rightText: Self.dateText(entry.rightModificationDate))
                MetadataItem(label: "パーミッション", leftText: Self.posixText(entry.leftPOSIX), rightText: Self.posixText(entry.rightPOSIX))
            }
        }
        .font(.caption2)
    }

    private static func sizeText(_ size: Int64) -> String {
        size < 0 ? "—" : ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    private static func dateText(_ date: Date?) -> String {
        guard let date else { return "—" }
        return Self.dateFormatter.string(from: date)
    }

    private static let dateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
        return formatter
    }()

    private static func posixText(_ meta: POSIXBasicMetadata?) -> String {
        guard let permissions = meta?.permissions else { return "—" }
        return String(format: "%04o", permissions)
    }
}

/// リネーム/移動候補の表示(片側のみファイルにのみ設定される)。
private struct RenameCandidateBar: View {
    let candidate: RenameCandidate

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: candidate.isUniqueMatch ? "checkmark.circle.fill" : "questionmark.circle")
                .foregroundColor(candidate.isUniqueMatch ? .green : .orange)
            Text(candidate.isUniqueMatch ? "リネーム/移動の候補(確定)" : "リネーム/移動の候補")
            Text("→ \(candidate.counterpartPath)")
                .lineLimit(1)
                .truncationMode(.middle)
            if !candidate.isUniqueMatch {
                Text("(同内容 \(candidate.totalMatches)件)")
                    .foregroundColor(.secondary)
            }
        }
    }
}

private struct MetadataItem: View {
    let label: String
    let leftText: String
    let rightText: String

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(label).foregroundColor(.secondary)
            Text("左: \(leftText)").lineLimit(1)
            Text("右: \(rightText)").lineLimit(1)
        }
    }
}

/// 整列済み行の横並び描画。差分行ジャンプ・検索・コンテキスト折りたたみ・横スクロールに対応する。
private struct DiffRowsView: View {
    let rows: [DiffRow]
    let highlights: [Int: InlineHighlight]
    @Binding var expandedGroups: Set<Int>
    @Binding var showAllContextRows: Bool
    let currentDifferenceID: Int?
    let searchHitRowIDs: Set<Int>
    let currentSearchHitID: Int?
    let onExpandGroup: (Int) -> Void
    @Binding var scrollTarget: Int?

    var body: some View {
        let items = DiffDisplay.displayItems(
            rows: rows,
            expandedGroupIDs: expandedGroups,
            showAllContextRows: showAllContextRows
        )
        ScrollViewReader { proxy in
            ScrollView([.vertical, .horizontal]) {
                LazyVStack(spacing: 0) {
                    ForEach(items) { item in
                        switch item {
                        case .row(let row):
                            DiffRowView(
                                row: row,
                                highlight: highlights[row.id],
                                isCurrentDifference: row.id == currentDifferenceID,
                                isSearchMatch: searchHitRowIDs.contains(row.id),
                                isCurrentSearchHit: row.id == currentSearchHitID
                            )
                            .id(item.id)
                        case .collapsed(let group):
                            CollapsedRow(group: group) {
                                onExpandGroup(group.id)
                            }
                            .id(item.id)
                        }
                    }
                }
            }
            .onChange(of: scrollTarget) { target in
                guard let target else { return }
                scrollTarget = nil
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                    withAnimation(.easeInOut(duration: 0.15)) {
                        proxy.scrollTo(target, anchor: .center)
                    }
                }
            }
        }
        .background(Color(nsColor: .textBackgroundColor))
    }
}

/// 折りたたまれたコンテキスト行ブロックのヘッダ(「N行を表示」)。クリックで展開する。
private struct CollapsedRow: View {
    let group: DiffDisplay.FoldGroup
    let onExpand: () -> Void

    var body: some View {
        HStack {
            Button(action: onExpand) {
                Label("\(group.count) 行を表示", systemImage: "chevron.right.circle")
            }
            .buttonStyle(.plain)
            .font(.caption)
            .foregroundColor(.secondary)
            Spacer()
        }
        .padding(.vertical, 2)
        .padding(.horizontal, 10)
        .background(Color(nsColor: .controlBackgroundColor))
    }
}

private struct DiffRowView: View {
    let row: DiffRow
    var highlight: InlineHighlight? = nil
    var isCurrentDifference = false
    var isSearchMatch = false
    var isCurrentSearchHit = false

    var body: some View {
        HStack(alignment: .top, spacing: 0) {
            LineCell(
                number: row.leftNumber,
                text: row.leftText,
                background: leftBackground,
                highlightRanges: highlight?.leftRanges
            )
            Divider()
            LineCell(
                number: row.rightNumber,
                text: row.rightText,
                background: rightBackground,
                highlightRanges: highlight?.rightRanges
            )
        }
        .fixedSize(horizontal: true, vertical: true)
        .background(searchHighlightBackground)
        .overlay(currentDifferenceBorder)
    }

    // 配色: 追加=緑系、削除=赤系、変更=黄系(SPEC.md §2.6)
    private var leftBackground: Color {
        switch row.kind {
        case .context: return .clear
        case .removed: return .red.opacity(0.16)
        case .added: return Color.secondary.opacity(0.06)
        case .modified: return .yellow.opacity(0.18)
        }
    }

    private var rightBackground: Color {
        switch row.kind {
        case .context: return .clear
        case .removed: return Color.secondary.opacity(0.06)
        case .added: return .green.opacity(0.16)
        case .modified: return .yellow.opacity(0.18)
        }
    }

    private var searchHighlightBackground: Color {
        guard isSearchMatch else { return .clear }
        return isCurrentSearchHit ? Color.blue.opacity(0.30) : Color.blue.opacity(0.12)
    }

    @ViewBuilder
    private var currentDifferenceBorder: some View {
        if isCurrentDifference {
            RoundedRectangle(cornerRadius: 2)
                .strokeBorder(Color.accentColor, lineWidth: 1.5)
        }
    }
}

private struct LineCell: View {
    let number: Int?
    let text: String?
    let background: Color
    var highlightRanges: [Range<Int>]? = nil

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            Text(number.map(String.init) ?? "")
                .font(.system(.caption, design: .monospaced))
                .foregroundColor(.secondary)
                .frame(width: 42, alignment: .trailing)
            content
        }
        .padding(.vertical, 1)
        .padding(.horizontal, 4)
        .background(background)
    }

    @ViewBuilder
    private var content: some View {
        let base = Text(text ?? "")
            .font(.system(.callout, design: .monospaced))
            .textSelection(.enabled)
        if let text, let highlightRanges, !highlightRanges.isEmpty {
            highlightedText(text, ranges: highlightRanges)
                .font(.system(.callout, design: .monospaced))
                .textSelection(.enabled)
                .frame(minWidth: 320, alignment: .leading)
        } else {
            base
                .frame(minWidth: 320, alignment: .leading)
        }
    }

    /// 変更範囲を太字+背景で強調したテキストを返す。範囲外インデックスは安全に無視する
    private func highlightedText(_ text: String, ranges: [Range<Int>]) -> Text {
        let chars = Array(text)
        var result = Text("")
        var index = 0
        for range in ranges {
            let lower = min(max(range.lowerBound, 0), chars.count)
            let upper = min(max(range.upperBound, 0), chars.count)
            guard lower < upper, index <= lower else { continue }
            if index < lower {
                result = result + Text(String(chars[index..<lower]))
            }
            result = result + Text(String(chars[lower..<upper]))
                .bold()
                .foregroundColor(.red)
            index = upper
        }
        if index < chars.count {
            result = result + Text(String(chars[index...]))
        }
        return result
    }
}
