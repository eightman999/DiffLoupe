// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較オプション群。内容比較の正規化モード(ContentComparison)と、
// ユーザー定義の除外glob(GlobPattern)を定義する。フォルダ分類(FolderComparator)と
// 詳細差分(LineDiff)の両方から共用できる設計(SPEC §2.x WinMerge互換土台)

import Foundation

/// 内容比較モード。WinMerge 互換の正規化フラグの組で、全てオフ=厳密比較(既定)。
/// フォルダ分類と詳細差分の両方で同じ正規化規則を共用する。
public struct ContentComparison: Sendable, Hashable {
    /// 改行コード無視(CRLF / CR / LF を同一視)
    public var ignoreLineEndings = false
    /// 行末の空白無視(スペース・タブ等)
    public var ignoreTrailingWhitespace = false
    /// 連続空白の正規化(タブ・複数スペースを単一スペースに)
    public var normalizeWhitespace = false
    /// 大文字小文字無視
    public var ignoreCase = false
    /// 構造化比較モード(JSON/XML/CSV)。既定 disabled(既存挙動と同一)。
    /// 有効時は JSON のキー順・XML の属性順・CSV の引用/区切りなどの表記揺れを
    /// 正規化してから比較する。パース失敗時は既存のテキスト/バイト比較へフォールバックする。
    public var structuredMode: StructuredComparisonMode = .disabled

    public init(
        ignoreLineEndings: Bool = false,
        ignoreTrailingWhitespace: Bool = false,
        normalizeWhitespace: Bool = false,
        ignoreCase: Bool = false,
        structuredMode: StructuredComparisonMode = .disabled
    ) {
        self.ignoreLineEndings = ignoreLineEndings
        self.ignoreTrailingWhitespace = ignoreTrailingWhitespace
        self.normalizeWhitespace = normalizeWhitespace
        self.ignoreCase = ignoreCase
        self.structuredMode = structuredMode
    }

    /// すべての正規化が無効 = 厳密比較(既定)。構造化モード有効時は false(正規化扱い)。
    public var isStrict: Bool {
        !ignoreLineEndings && !ignoreTrailingWhitespace && !normalizeWhitespace && !ignoreCase
            && structuredMode == .disabled
    }

    /// 設定内容を一意に表す文字列。詳細ビューの再読み込みトリガー(タスクid)に用いる。
    /// 同じシグネチャ = 同じ正規化挙動なので、再表示不要と判定できる。
    /// 後方互換のため、構造化モード無効時は従来どおり4桁のフラグ文字列を返す。
    public var settingsSignature: String {
        let flags = "\(ignoreLineEndings ? 1 : 0)\(ignoreTrailingWhitespace ? 1 : 0)\(normalizeWhitespace ? 1 : 0)\(ignoreCase ? 1 : 0)"
        guard structuredMode != .disabled else { return flags }
        return "\(flags)|\(structuredMode.rawValue)"
    }

    /// バイナリ判定に使う先頭バイト数(TextFileLoader と同一)
    static let binarySniffLength = 8192

    // MARK: - フォルダ分類用(バイト列)

    /// 比較用にバイト列をテキスト正規化する。厳密比較ではそのまま返す。
    /// バイナリ(NULバイトを含む)または UTF-8 としてデコード不能な場合は nil を返し、
    /// 呼び出し側はバイト厳密比較にフォールバックする(内容相違の見逃しを防ぐ保守側)。
    /// 構造化モードのカノニカル比較は structuredCanonicalPair で左右両側を同時判定し、
    /// 本関数は構造化モード無効化後のテキスト/バイト正規化のみを担う。
    func normalizedData(_ data: Data) -> Data? {
        if isStrict { return data }
        if data.prefix(Self.binarySniffLength).contains(0) { return nil }
        guard let text = String(data: data, encoding: .utf8) else { return nil }
        return normalizedText(text).data(using: .utf8)
    }

    /// 左右両側の構造化正規化を同時に判定する。両側とも正規化成功した場合のみ
    /// カノニカル文字列ペアを返し、片側でもパース失敗した場合は nil を返す。
    /// 呼び出し側は nil のとき structuredMode を無効化したテキスト/バイト比較へ
    /// 左右両方をフォールバックする(成功側のcanonicalと失敗側のrawを混在比較しない)。
    /// - Parameters:
    ///   - left: 左側の対象バイト列
    ///   - leftURL: 左側の拡張子判定用(automatic モードで使用)
    ///   - right: 右側の対象バイト列
    ///   - rightURL: 右側の拡張子判定用(automatic モードで使用)
    func structuredCanonicalPair(
        _ left: Data, url leftURL: URL?,
        _ right: Data, url rightURL: URL?
    ) -> (leftText: String, rightText: String)? {
        guard structuredMode != .disabled else { return nil }
        let mode = structuredMode
        let leftResult = StructuredNormalizer.normalize(left, url: leftURL, mode: mode)
        let rightResult = StructuredNormalizer.normalize(right, url: rightURL, mode: mode)
        guard leftResult.didNormalize, rightResult.didNormalize,
              let leftText = leftResult.normalizedText, let rightText = rightResult.normalizedText else {
            return nil
        }
        return (leftText, rightText)
    }

    // MARK: - 詳細差分用(テキスト)

    /// テキスト全体を行単位で正規化する。LineDiff が左右の行列の差分計算前に適用する。
    func normalizedText(_ text: String) -> String {
        var base = text
        if ignoreLineEndings || normalizeWhitespace {
            base = base.replacingOccurrences(of: "\r\n", with: "\n")
            base = base.replacingOccurrences(of: "\r", with: "\n")
        }
        let normalizedLines = base
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map { normalizedLine(String($0)) }
        return normalizedLines.joined(separator: "\n")
    }

    /// 1行の正規化(行は改行文字を含まない前提)。
    /// 行末のCRは ignoreLineEndings/normalizeWhitespace の段階で除去済み。
    func normalizedLine(_ line: String) -> String {
        var result = line
        if ignoreTrailingWhitespace || normalizeWhitespace {
            result = Self.trimmingTrailingWhitespace(result)
        }
        if normalizeWhitespace {
            // 空白区切りで分割→単一スペースで再結合(先頭・末尾も除去、内部連続空白を1つに)
            result = result.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ")
        }
        if ignoreCase {
            result = result.lowercased()
        }
        return result
    }

    private static func trimmingTrailingWhitespace(_ string: String) -> String {
        var end = string.endIndex
        while end > string.startIndex {
            let previous = string.index(before: end)
            guard string[previous].isWhitespace else { break }
            end = previous
        }
        return String(string[..<end])
    }
}

/// ユーザー定義の除外globパターン。ファイル名(区切りなし)または相対パス(区切りあり)に照合する。
/// 対応ワイルドカード: `*`=区切りをまたがない任意列、`?`=区切りをまたがない1文字、
/// `**`=区切りをまたぐ任意列(0個以上)。ディレクトリに一致すればサブツリーごと枝刈りされる。
struct GlobPattern: Sendable, Hashable {
    let pattern: String

    private let hasSeparator: Bool
    private let tokens: [Substring]

    init(_ pattern: String) {
        self.pattern = pattern
        self.hasSeparator = pattern.contains("/")
        self.tokens = pattern.split(separator: "/")
    }

    /// エントリが除外対象かを判定する。
    /// - Parameter name: エントリ名(最終パス成分)
    /// - Parameter relativePath: ルートからの相対パス(NFC正規化済み・"/"区切り)
    func matches(name: String, relativePath: String) -> Bool {
        if hasSeparator {
            return Self.match(tokens: tokens[...], path: ArraySlice(relativePath.split(separator: "/")))
        }
        return Self.match(tokens: tokens[...], path: ArraySlice(name.split(separator: "/")))
    }

    // MARK: - セグメント単位のマッチング(DP)

    private static func match(tokens: ArraySlice<Substring>, path: ArraySlice<Substring>) -> Bool {
        if tokens.isEmpty {
            return path.isEmpty
        }
        if path.isEmpty {
            // 残りトークンが全て ** なら空と一致しうる
            return tokens.allSatisfy { $0 == "**" }
        }
        let token = tokens[tokens.startIndex]
        if token == "**" {
            // ** は0個以上のセグメントに一致
            return match(tokens: tokens.dropFirst(), path: path)
                || match(tokens: tokens, path: path.dropFirst())
        }
        if containsWildcard(token) {
            guard singleTokenMatch(token, path[path.startIndex]) else { return false }
            return match(tokens: tokens.dropFirst(), path: path.dropFirst())
        }
        guard token == path[path.startIndex] else { return false }
        return match(tokens: tokens.dropFirst(), path: path.dropFirst())
    }

    private static func containsWildcard(_ token: Substring) -> Bool {
        token.contains("*") || token.contains("?")
    }

    /// 単一トークン内の `*`(任意列)/`?`(任意1文字)マッチ。`/` はトークンに含まれない。
    /// 貪欲+バックトラックによる標準的な反復実装。
    private static func singleTokenMatch(_ token: Substring, _ candidate: Substring) -> Bool {
        let pattern = Array(token)
        let string = Array(candidate)
        var si = 0
        var pi = 0
        var starIndex = -1
        var starMatch = 0

        while si < string.count {
            if pi < pattern.count, pattern[pi] == "?" || pattern[pi] == string[si] {
                si += 1
                pi += 1
            } else if pi < pattern.count, pattern[pi] == "*" {
                starIndex = pi
                starMatch = si
                pi += 1
            } else if starIndex != -1 {
                // '*' を1文字多く消費させる(バックトラック)
                starMatch += 1
                si = starMatch
                pi = starIndex + 1
            } else {
                return false
            }
        }
        while pi < pattern.count, pattern[pi] == "*" { pi += 1 }
        return pi == pattern.count
    }
}
