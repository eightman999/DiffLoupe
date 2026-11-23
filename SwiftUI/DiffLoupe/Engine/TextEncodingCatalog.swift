// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 手動で選べる文字コードの一覧(group ごとにまとめて返す)と、
// 詳細表示で「そのエントリを表示している間だけ」有効な文字コード指定の状態(TextEncodingOverrides)。
// 一覧の実体は Rust コア(diffloupe-ffi)が持ち、Swift 側は薄いファサード。指定は永続化しない。

import Foundation

/// 手動選択できる文字コード1件。`name` は読み込み関数へ渡す識別名、`displayName` はメニュー表示名。
public struct TextEncodingOption: Sendable, Equatable, Identifiable {
    public let name: String
    public let displayName: String
    public let group: String
    public var id: String { name }
}

/// 同じ group の文字コードのまとまり(メニューの区切り単位)
public struct TextEncodingGroup: Sendable, Equatable, Identifiable {
    public let name: String
    public let options: [TextEncodingOption]
    public var id: String { name }
}

public enum TextEncodingCatalog {

    /// 対応する全文字コード(Rust 側が決めた表示順)
    public static let all: [TextEncodingOption] = diffloupeTextSupportedEncodings().map {
        TextEncodingOption(name: $0.name, displayName: $0.displayName, group: $0.group)
    }

    /// group ごとにまとめた一覧。group の出現順・group 内の順序は `all` のまま。
    public static var groups: [TextEncodingGroup] { grouped(all) }

    public static func grouped(_ options: [TextEncodingOption]) -> [TextEncodingGroup] {
        var order: [String] = []
        var buckets: [String: [TextEncodingOption]] = [:]
        for option in options {
            if buckets[option.group] == nil { order.append(option.group) }
            buckets[option.group, default: []].append(option)
        }
        return order.map { TextEncodingGroup(name: $0, options: buckets[$0] ?? []) }
    }

    public static func option(named name: String) -> TextEncodingOption? {
        all.first { $0.name == name }
    }

    /// 文字コード名(大文字小文字は無視)を一覧の正規名へ。未対応は nil(Rust CLI の canonical_encoding_name と同じ規則)。
    public static func canonicalName(_ name: String) -> String? {
        if let exact = all.first(where: { $0.name == name }) { return exact.name }
        return all.first { $0.name.lowercased() == name.lowercased() }?.name
    }

    /// `--list-encodings` の出力行(名前<TAB>表示名)
    public static var listLines: [String] { all.map { "\($0.name)\t\($0.displayName)" } }

    /// 識別名からメニュー表示名を引く。一覧に無ければ識別名をそのまま返す。
    public static func displayName(for name: String) -> String {
        option(named: name)?.displayName ?? name
    }
}

/// 詳細表示での文字コードの手動指定(左右別)。指定は「選んだエントリ」に紐づく一時的な状態で、
/// 別のエントリを表示すると `effective(for:)` が自動判定(nil)を返す(明示的なリセット処理は不要)。
public struct TextEncodingOverrides: Equatable, Sendable {
    public enum Side: Sendable { case left, right }

    public private(set) var entryPath: String?
    public private(set) var left: String?
    public private(set) var right: String?

    public init() {}

    /// `entryPath` のエントリの `side` 側を `encoding`(nil=自動判定)にする。
    /// 別エントリの指定が残っていたら先に捨てる。
    public mutating func select(_ encoding: String?, side: Side, entryPath path: String?) {
        if entryPath != path {
            left = nil
            right = nil
            entryPath = path
        }
        switch side {
        case .left: left = encoding
        case .right: right = encoding
        }
    }

    /// `path` のエントリを表示するときの指定。別エントリなら (nil, nil)=自動判定。
    public func effective(for path: String?) -> (left: String?, right: String?) {
        guard path != nil, path == entryPath else { return (nil, nil) }
        return (left, right)
    }
}

/// 3-wayの3ファイル(ベース・左・右)それぞれの文字コード指定。ファイルのパスに紐づく一時的な状態で、
/// 別のファイルを選び直すとそのファイルの指定は自動判定に戻る。保存しない。
public struct ThreeWayEncodingOverrides: Equatable, Sendable {
    public enum Role: Sendable, Hashable { case base, left, right }

    private var entries: [Role: (path: String, encoding: String)] = [:]

    public init() {}

    public static func == (lhs: Self, rhs: Self) -> Bool {
        Set(lhs.entries.keys) == Set(rhs.entries.keys)
            && lhs.entries.allSatisfy { rhs.entries[$0.key]?.path == $0.value.path && rhs.entries[$0.key]?.encoding == $0.value.encoding }
    }

    public mutating func select(_ encoding: String?, role: Role, path: String) {
        if let encoding { entries[role] = (path, encoding) } else { entries[role] = nil }
    }

    /// `role` のファイル(`path`)に有効な指定。別のファイルに差し替わっていたら nil(自動判定)。
    public func effective(_ role: Role, path: String?) -> String? {
        guard let path, let entry = entries[role], entry.path == path else { return nil }
        return entry.encoding
    }
}

/// 3-wayで1ファイルを読んだ結果(UI非依存・読み取り専用)。
public struct ThreeWayLoadedText: Sendable, Equatable {
    public let text: String
    public let encodingName: String
    /// 指定した文字コードで読めないバイトを U+FFFD に置換したか
    public let hadReplacements: Bool
}

public enum ThreeWayTextLoader {
    public enum LoadError: LocalizedError, Sendable, Equatable {
        case binary(label: String, fileName: String)
        case tooLarge(label: String)

        public var errorDescription: String? {
            switch self {
            case .binary(let label, let fileName): return "\(label)「\(fileName)」はテキストとして読み込めません"
            case .tooLarge(let label): return "\(label)ファイルが大きすぎて読み込めません"
            }
        }
    }

    /// 1ファイルを読む。`encoding` が nil なら自動判定、指定ありならその文字コードで読む(そのファイルだけ)。
    public static func load(url: URL, encoding: String?, label: String) throws -> ThreeWayLoadedText {
        if let encoding {
            switch try TextFileLoader.load(url: url, encoding: encoding) {
            case .text(let text, let name, let replaced):
                return ThreeWayLoadedText(text: text, encodingName: name, hadReplacements: replaced)
            case .tooLarge:
                throw LoadError.tooLarge(label: label)
            }
        }
        switch try TextFileLoader.load(url: url) {
        case .text(let text, let name):
            return ThreeWayLoadedText(text: text, encodingName: name, hadReplacements: false)
        case .binary:
            throw LoadError.binary(label: label, fileName: url.lastPathComponent)
        case .tooLarge:
            throw LoadError.tooLarge(label: label)
        }
    }
}
