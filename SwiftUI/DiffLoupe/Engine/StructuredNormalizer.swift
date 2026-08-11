// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: JSON / XML / CSV・TSV の構造化正規化(比較用の決定論的カノニカル形式への変換)。
// Foundation のみに依存し、JSONSerialization / XMLParser でパースする。
// フォルダ分類(FolderComparator)と詳細差分(LineDiff)の両方で同一の正規化結果を使う。
//
// 正規化に成功しなかった場合(didNormalize == false)は、既存のテキスト/バイト正規化へ
// フォールバックする。構造化パースが成功していないのに「構造化で一致」と判定しないための
// 安全側の契約(normalize が返す notice を UI が警告表示に使う)。
//
// 制約:
// - パース上限 maxParseBytes(8 MiB)を超えるデータは構造化パースしない(フォールバック)
// - JSON: オブジェクトのキーをソート、配列は順序を保持、決定論的に整形
// - XML : 属性をソート、2スペース字下げ、インデント専用の空白のみ除去
// - CSV : RFC4180 の引用符規則を解釈し、区切り文字(カンマ/タブ/セミコロン)を自動検出

import Foundation

/// 構造化フォーマット。URL拡張子(.json/.xml/.csv/.tsv)または明示モードから決まる。
public enum StructuredFormat: String, Sendable, Hashable, CaseIterable {
    case json
    case xml
    case csv

    public var displayName: String {
        switch self {
        case .json: return "JSON"
        case .xml: return "XML"
        case .csv: return "CSV"
        }
    }

    /// 拡張子からフォーマットを判定する。.tsv は CSV(タブ区切り)として扱う。未知なら nil。
    public init?(urlExtension: String) {
        switch urlExtension.lowercased() {
        case "json": self = .json
        case "xml": self = .xml
        case "csv": self = .csv
        case "tsv": self = .csv
        default: return nil
        }
    }
}

/// 構造化比較モード。既定は disabled(既存挙動と同一)。
public enum StructuredComparisonMode: String, Sendable, Hashable, CaseIterable {
    /// 構造化比較を行わない(バイト/テキスト正規化のみ)
    case disabled
    /// 拡張子(.json/.xml/.csv/.tsv)から自動判定する。未知なら従来どおりテキスト比較へ
    case automatic
    case json
    case xml
    case csv

    public var displayName: String {
        switch self {
        case .disabled: return "オフ"
        case .automatic: return "自動(拡張子で判定)"
        case .json: return "JSON"
        case .xml: return "XML"
        case .csv: return "CSV"
        }
    }
}

/// 構造化正規化の結果。didNormalize == true のとき normalizedText が有効。
/// didNormalize == false のときは notice にフォールバック理由を載せ、呼び出し側は
/// 既存のテキスト/バイト正規化へフォールバックする(構造化一致を主張しない)。
public struct StructuredNormalizationResult: Sendable, Equatable {
    public let didNormalize: Bool
    /// 正規化に使った(または試みた)フォーマット。自動判定で未知拡張子の場合は nil
    public let format: StructuredFormat?
    /// 正規化後の決定論的テキスト(didNormalize == true のときのみ非 nil)
    public let normalizedText: String?
    /// フォールバック理由などの通知(didNormalize == false のとき設定されることがある)
    public let notice: String?

    public init(
        didNormalize: Bool,
        format: StructuredFormat?,
        normalizedText: String?,
        notice: String?
    ) {
        self.didNormalize = didNormalize
        self.format = format
        self.normalizedText = normalizedText
        self.notice = notice
    }
}

public enum StructuredNormalizeError: Error, Equatable {
    case jsonParse(String)
    case xmlParse(String)
    case csvParse(String)
}

/// 構造化正規化エンジン。Foundation のみに依存する(パースに任意のスレッドで使える)。
public enum StructuredNormalizer {

    /// 構造化パースの上限バイト数。超過はパースせずフォールバックする。
    public static let maxParseBytes = 8 * 1024 * 1024

    // MARK: - 公開API

    /// データを指定モードで正規化する。
    /// - Parameters:
    ///   - data: 対象バイト列(UTF-8 前提)
    ///   - url: 拡張子判定用(automatic モードで使用)
    ///   - mode: 構造化比較モード
    /// - Returns: 正規化結果。失敗時は didNormalize == false でフォールバックを指示する
    public static func normalize(
        _ data: Data,
        url: URL?,
        mode: StructuredComparisonMode
    ) -> StructuredNormalizationResult {
        if mode == .disabled {
            return StructuredNormalizationResult(
                didNormalize: false, format: nil, normalizedText: nil, notice: nil)
        }

        let format: StructuredFormat?
        switch mode {
        case .automatic:
            format = url.flatMap { StructuredFormat(urlExtension: $0.pathExtension) }
        case .json, .xml, .csv:
            format = StructuredFormat(rawValue: mode.rawValue)
        case .disabled:
            format = nil
        }
        guard let format else {
            return StructuredNormalizationResult(
                didNormalize: false, format: nil, normalizedText: nil, notice: nil)
        }

        // 共通入口で UTF-8 BOM(EF BB BF)を除去してから JSON/XML/CSV すべてを解析する。
        // FolderComparator の Data(contentsOf:) 直接読込経路では BOM が文字列に残り、
        // 先頭フィールドに U+FEFF が付いたまま CSV パースされて BOM なし CSV と相違になる。
        // 上限判定は除去後の実内容で行う(8 MiB 上限の意味とフォールバック契約は維持)。
        let content = Self.strippingUTF8BOM(data)
        guard content.count <= maxParseBytes else {
            return StructuredNormalizationResult(
                didNormalize: false,
                format: format,
                normalizedText: nil,
                notice: "\(format.displayName)パース上限(8 MiB)を超えているため、元のテキストで比較します"
            )
        }

        do {
            let text = try normalizedText(data: content, format: format)
            return StructuredNormalizationResult(
                didNormalize: true, format: format, normalizedText: text, notice: nil)
        } catch is StructuredNormalizeError {
            return StructuredNormalizationResult(
                didNormalize: false,
                format: format,
                normalizedText: nil,
                notice: "\(format.displayName)として解析できませんでした(元のテキストで比較します)"
            )
        } catch {
            return StructuredNormalizationResult(
                didNormalize: false,
                format: format,
                normalizedText: nil,
                notice: "\(format.displayName)の解析に失敗しました(元のテキストで比較します)"
            )
        }
    }

    // MARK: - 共通前処理

    /// UTF-8 BOM(EF BB BF)を除去する。BOM が無ければ入力をそのまま返す。
    private static func strippingUTF8BOM(_ data: Data) -> Data {
        guard data.count >= 3,
              data[data.startIndex] == 0xEF,
              data[data.startIndex + 1] == 0xBB,
              data[data.startIndex + 2] == 0xBF else {
            return data
        }
        return data.dropFirst(3)
    }

    // MARK: - フォーマット別正規化

    private static func normalizedText(data: Data, format: StructuredFormat) throws -> String {
        switch format {
        case .json: return try normalizeJSON(data: data)
        case .xml: return try normalizeXML(data: data)
        case .csv: return try normalizeCSV(data: data)
        }
    }

    // MARK: - JSON

    private static func normalizeJSON(data: Data) throws -> String {
        let object: Any
        do {
            object = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        } catch {
            throw StructuredNormalizeError.jsonParse(error.localizedDescription)
        }
        return renderJSON(object, depth: 0)
    }

    /// JSON値を決定論的に描画する。オブジェクトのキーはソートし、配列は順序を保持する。
    private static func renderJSON(_ value: Any, depth: Int) -> String {
        let indent = String(repeating: "  ", count: depth)
        let itemIndent = String(repeating: "  ", count: depth + 1)

        if let dictionary = value as? [String: Any] {
            if dictionary.isEmpty { return "{}" }
            let keys = dictionary.keys.sorted()
            let items = keys.map { key in
                itemIndent + jsonString(key) + " : " + renderJSON(dictionary[key]!, depth: depth + 1)
            }
            return "{\n" + items.joined(separator: ",\n") + "\n" + indent + "}"
        }
        if let array = value as? [Any] {
            if array.isEmpty { return "[]" }
            let items = array.map { itemIndent + renderJSON($0, depth: depth + 1) }
            return "[\n" + items.joined(separator: ",\n") + "\n" + indent + "]"
        }
        if let string = value as? String {
            return jsonString(string)
        }
        if let number = value as? NSNumber {
            return numberString(number)
        }
        if value is NSNull {
            return "null"
        }
        // NSNumber 以外の数値ブリッジ(NSString 等)は値ベースで描画する
        if let bool = value as? Bool {
            return bool ? "true" : "false"
        }
        return "null"
    }

    /// NSNumber を JSON リテラルへ変換する。Bool と数値を区別し、実数は
    /// IEEE-754 の値を往復復元できる桁(最大17桁)で描画する。
    private static func numberString(_ number: NSNumber) -> String {
        if CFGetTypeID(number) == CFBooleanGetTypeID() {
            return number.boolValue ? "true" : "false"
        }
        switch CFNumberGetType(number) {
        case .floatType, .float32Type, .doubleType, .float64Type, .cgFloatType:
            return renderFloating(number.doubleValue)
        default:
            return number.stringValue
        }
    }

    /// 実数を JSON リテラルへ変換する。String(Double) は最短往復表現を使うため、
    /// 0.1234567 と 0.1234568 のような近接した値も区別でき、ロケール非依存で常に '.'
    /// を使う(決定論的)。旧実装の String(format: "%g") は既定6桁に丸め、値の区別を
    /// 損なうだけでなくユーザーロケールの小数点にも依存していた。
    /// %g 互換の既存挙動として、整数値の double は "1.0" ではなく "1" と描画する。
    private static func renderFloating(_ value: Double) -> String {
        let text = String(value)
        if value == value.rounded(), text.hasSuffix(".0") {
            return String(text.dropLast(2))
        }
        return text
    }

    /// 文字列を JSON の `"..."` リテラルへ変換する(JSONSerialization による標準エスケープ)。
    /// トップレベル文字列を直接渡すとNSJSONSerializationが例外を投げるため、
    /// 単要素配列で包んでから外側の `[` `]` を取り除く。
    private static func jsonString(_ string: String) -> String {
        guard
            let data = try? JSONSerialization.data(withJSONObject: [string], options: []),
            let wrapped = String(data: data, encoding: .utf8),
            wrapped.count >= 2,
            wrapped.hasPrefix("["), wrapped.hasSuffix("]")
        else {
            return "\"\""
        }
        return String(wrapped.dropFirst().dropLast())
    }

    // MARK: - XML

    private static func normalizeXML(data: Data) throws -> String {
        let builder = XMLTreeBuilder()
        let parser = XMLParser(data: data)
        parser.delegate = builder
        guard parser.parse(), builder.parseError == nil, !builder.documentNodes.isEmpty else {
            let detail = builder.parseError?.localizedDescription ?? "文書が空です"
            throw StructuredNormalizeError.xmlParse(detail)
        }
        var lines: [String] = []
        for node in builder.documentNodes {
            renderXML(node: node, depth: 0, lines: &lines)
        }
        return lines.joined(separator: "\n") + "\n"
    }

    private static func renderXML(node: XMLNode, depth: Int, lines: inout [String]) {
        let indent = String(repeating: "  ", count: depth)
        switch node {
        case .text(let text):
            lines.append(indent + escapeText(text))
        case .comment(let comment):
            lines.append(indent + "<!--" + comment + "-->")
        case .pi(let target, let data):
            let content = data.isEmpty ? "" : " " + data
            lines.append(indent + "<?" + target + content + "?>")
        case .element(let name, let attributes, let children):
            let attrString = attributes.map { " \($0.0)=\"\(escapeAttr($0.1))\"" }.joined()
            let filtered = filteredChildren(children)
            if filtered.isEmpty {
                lines.append(indent + "<\(name)\(attrString)/>")
                return
            }
            if filtered.allSatisfy({ if case .text = $0 { return true } else { return false } }) {
                // テキストのみの要素は1行で出力
                let content = filtered.map { node -> String in
                    if case .text(let text) = node { return escapeText(text) }
                    return ""
                }.joined()
                lines.append(indent + "<\(name)\(attrString)>\(content)</\(name)>")
                return
            }
            lines.append(indent + "<\(name)\(attrString)>")
            for child in filtered {
                renderXML(node: child, depth: depth + 1, lines: &lines)
            }
            lines.append(indent + "</\(name)>")
        }
    }

    /// インデント専用の空白のみを除去する。
    /// 要素子を持つ要素(要素専用コンテンツ)では、空白だけのテキストノードを落とす。
    /// 意味のあるテキストが混在する混合コンテンツではすべて保持する(安全側)。
    private static func filteredChildren(_ children: [XMLNode]) -> [XMLNode] {
        let hasElementChild = children.contains { node in
            if case .element = node { return true } else { return false }
        }
        let hasSignificantText = children.contains { node in
            if case .text(let text) = node {
                return !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            }
            return false
        }
        guard hasElementChild, !hasSignificantText else { return children }
        return children.filter { node in
            if case .text(let text) = node {
                return !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            }
            return true
        }
    }

    private static func escapeText(_ text: String) -> String {
        text
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
    }

    private static func escapeAttr(_ value: String) -> String {
        escapeText(value).replacingOccurrences(of: "\"", with: "&quot;")
    }

    // MARK: - CSV / TSV

    private static func normalizeCSV(data: Data) throws -> String {
        guard let text = String(data: data, encoding: .utf8) else {
            throw StructuredNormalizeError.csvParse("UTF-8としてデコードできません")
        }
        let delimiter = detectCSVDelimiter(text)
        let rows = try parseCSV(text, delimiter: delimiter)
        guard !rows.isEmpty else {
            throw StructuredNormalizeError.csvParse("行がありません")
        }
        return renderCSV(rows)
    }

    /// 区切り文字を最初の行の出現数で判定する(引用符の外側のみ数える)。
    /// 同数の場合は カンマ > タブ > セミコロン の優先順。
    static func detectCSVDelimiter(_ text: String) -> Character {
        let candidates: [Character] = [",", "\t", ";"]
        var counts: [Character: Int] = [",": 0, "\t": 0, ";": 0]
        var inQuotes = false
        for character in text {
            if character == "\"" {
                inQuotes.toggle()
            } else if !inQuotes, counts[character] != nil {
                counts[character, default: 0] += 1
            }
            if character == "\n" { break }
        }
        var best: Character = ","
        var bestCount = -1
        for candidate in candidates {
            if let count = counts[candidate], count > bestCount {
                best = candidate
                bestCount = count
            }
        }
        return best
    }

    /// RFC4180 風のCSVパース。引用符フィールド・`""`エスケープ・引用内改行に対応し、
    /// 閉じられていない引用符はエラーにする。非引用フィールドの前後空白は除去する。
    static func parseCSV(_ text: String, delimiter: Character) throws -> [[String]] {
        var rows: [[String]] = []
        var row: [String] = []
        var field = ""
        var inQuotes = false
        var fieldWasQuoted = false
        var index = text.startIndex

        func closeField() {
            if !fieldWasQuoted {
                field = field.trimmingCharacters(in: .whitespaces)
            }
            row.append(field)
            field = ""
            fieldWasQuoted = false
        }
        func closeRow() {
            closeField()
            rows.append(row)
            row = []
        }

        while index < text.endIndex {
            let character = text[index]
            if inQuotes {
                if character == "\"" {
                    let next = text.index(after: index)
                    if next < text.endIndex, text[next] == "\"" {
                        field.append("\"")
                        index = text.index(after: next)
                    } else {
                        inQuotes = false
                        index = next
                    }
                } else {
                    field.append(character)
                    index = text.index(after: index)
                }
            } else {
                switch character {
                case "\"":
                    fieldWasQuoted = true
                    inQuotes = true
                    index = text.index(after: index)
                case delimiter:
                    closeField()
                    index = text.index(after: index)
                case "\n":
                    closeRow()
                    index = text.index(after: index)
                case "\r":
                    let next = text.index(after: index)
                    if next < text.endIndex, text[next] == "\n" {
                        closeRow()
                        index = text.index(after: next)
                    } else {
                        // 単独CR(旧Mac改行)も行区切りとして扱う
                        closeRow()
                        index = next
                    }
                default:
                    field.append(character)
                    index = text.index(after: index)
                }
            }
        }

        if inQuotes {
            throw StructuredNormalizeError.csvParse("閉じられていない引用符があります")
        }
        if !field.isEmpty || !row.isEmpty {
            closeField()
            rows.append(row)
        }
        return rows
    }

    /// 決定論的な固定幅テキストへ描画する。列幅は列内の最大幅、区切りは単一スペース。
    /// 空白・引用符を含むセルは `"..."`(内部の `"` は `""`)で引用し、曖昧さを排除する。
    static func renderCSV(_ rows: [[String]]) -> String {
        let columnCount = rows.map(\.count).max() ?? 0
        guard columnCount > 0 else { return "" }
        var widths = [Int](repeating: 0, count: columnCount)
        for row in rows {
            for (column, cell) in row.enumerated() where column < columnCount {
                widths[column] = max(widths[column], renderCSVCell(cell).count)
            }
        }
        var lines: [String] = []
        lines.reserveCapacity(rows.count)
        for row in rows {
            var cells: [String] = []
            cells.reserveCapacity(columnCount)
            for column in 0..<columnCount {
                let cell = column < row.count ? row[column] : ""
                let rendered = renderCSVCell(cell)
                cells.append(rendered.padding(toLength: widths[column], withPad: " ", startingAt: 0))
            }
            lines.append(cells.joined(separator: " "))
        }
        return lines.joined(separator: "\n") + "\n"
    }

    private static func renderCSVCell(_ cell: String) -> String {
        if cell.contains("\"") || cell.contains(" ") || cell.contains("\t") {
            return "\"" + cell.replacingOccurrences(of: "\"", with: "\"\"") + "\""
        }
        return cell
    }
}

// MARK: - XMLツリー構築

private enum XMLNode {
    case element(name: String, attributes: [(String, String)], children: [XMLNode])
    case text(String)
    case comment(String)
    case pi(target: String, data: String)
}

/// XMLParser のイベントからノードツリーを構築するデリゲート。
private final class XMLTreeBuilder: NSObject, XMLParserDelegate {
    var documentNodes: [XMLNode] = []
    var parseError: Error?

    private var stack: [(name: String, attributes: [(String, String)], children: [XMLNode])] = []
    private var currentText = ""

    private func append(_ node: XMLNode) {
        if let last = stack.indices.last {
            stack[last].children.append(node)
        } else {
            documentNodes.append(node)
        }
    }

    private func flushText() {
        guard !currentText.isEmpty else { return }
        let text = currentText
        currentText = ""
        append(.text(text))
    }

    func parser(
        _ parser: XMLParser,
        didStartElement elementName: String,
        namespaceURI: String?,
        qualifiedName qName: String?,
        attributes attributeDict: [String: String]
    ) {
        flushText()
        let sortedAttributes = attributeDict.sorted { $0.key < $1.key }
        stack.append((elementName, sortedAttributes, []))
    }

    func parser(
        _ parser: XMLParser,
        didEndElement elementName: String,
        namespaceURI: String?,
        qualifiedName qName: String?
    ) {
        flushText()
        guard let last = stack.popLast() else { return }
        let element = XMLNode.element(name: last.name, attributes: last.attributes, children: last.children)
        if let parent = stack.indices.last {
            stack[parent].children.append(element)
        } else {
            documentNodes.append(element)
        }
    }

    func parser(_ parser: XMLParser, foundCharacters string: String) {
        currentText += string
    }

    func parser(_ parser: XMLParser, foundCDATA CDATABlock: Data) {
        currentText += String(data: CDATABlock, encoding: .utf8) ?? ""
    }

    func parser(_ parser: XMLParser, foundComment comment: String) {
        flushText()
        append(.comment(comment))
    }

    func parser(
        _ parser: XMLParser,
        foundProcessingInstructionWithTarget target: String,
        data: String?
    ) {
        flushText()
        append(.pi(target: target, data: data ?? ""))
    }

    func parser(_ parser: XMLParser, parseErrorOccurred parseError: Error) {
        self.parseError = parseError
    }
}
