// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 比較オプション群。内容比較の正規化モード(ContentComparison)と、
// ユーザー定義の除外glob(GlobPattern)を定義する。フォルダ分類(FolderComparator)と
// 詳細差分(LineDiff)の両方から共用できる設計(SPEC §2.x WinMerge互換土台)。
// 実装は Rust コア(diffloupe-ffi)への薄いファサード。

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
        diffloupeNormalizedData(contentComparison: ffi, data: data)
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
        guard let pair = diffloupeStructuredCanonicalPair(
            contentComparison: ffi,
            left: left, leftUrl: leftURL?.path,
            right: right, rightUrl: rightURL?.path
        ) else { return nil }
        return (pair.leftText, pair.rightText)
    }

    // MARK: - 詳細差分用(テキスト)

    /// テキスト全体を行単位で正規化する。LineDiff が左右の行列の差分計算前に適用する。
    func normalizedText(_ text: String) -> String {
        diffloupeNormalizedText(contentComparison: ffi, text: text)
    }

    /// 1行の正規化(行は改行文字を含まない前提)。
    /// 行末のCRは ignoreLineEndings/normalizeWhitespace の段階で除去済み。
    func normalizedLine(_ line: String) -> String {
        diffloupeNormalizedLine(contentComparison: ffi, line: line)
    }
}

/// ユーザー定義の除外globパターン。ファイル名(区切りなし)または相対パス(区切りあり)に照合する。
/// 対応ワイルドカード: `*`=区切りをまたがない任意列、`?`=区切りをまたがない1文字、
/// `**`=区切りをまたぐ任意列(0個以上)。ディレクトリに一致すればサブツリーごと枝刈りされる。
struct GlobPattern: Sendable, Hashable {
    let pattern: String

    init(_ pattern: String) {
        self.pattern = pattern
    }

    /// エントリが除外対象かを判定する。
    /// - Parameter name: エントリ名(最終パス成分)
    /// - Parameter relativePath: ルートからの相対パス(NFC正規化済み・"/"区切り)
    func matches(name: String, relativePath: String) -> Bool {
        diffloupeGlobMatches(pattern: pattern, name: name, relativePath: relativePath)
    }
}

// MARK: - FFI 変換

extension ContentComparison {
    var ffi: FfiContentComparison {
        FfiContentComparison(
            ignoreLineEndings: ignoreLineEndings,
            ignoreTrailingWhitespace: ignoreTrailingWhitespace,
            normalizeWhitespace: normalizeWhitespace,
            ignoreCase: ignoreCase,
            structuredMode: structuredMode.ffi
        )
    }
}
