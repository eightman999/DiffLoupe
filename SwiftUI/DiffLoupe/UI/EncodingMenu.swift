// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 文字コードを選ぶメニュー(詳細表示・3-way・Proの3-wayで共用)。
// 項目は「自動判定」+ TextEncodingCatalog の group ごとの全文字コード。group はFFIが返すものをそのまま使い、
// 未知の group が増えても表示される。選択の保持は呼び出し側(一時的な状態)が行う。

import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

/// 左右それぞれの文字コードを選ぶメニュー。「自動判定」+ group ごとの全文字コード。
/// `selection` が現在の手動指定(nil=自動判定)で、チェックを付ける。`current` は適用中の文字コード表示名。
public struct EncodingMenu: View {
    let title: String
    let current: String?
    let selection: String?
    let automaticTitle: String
    let onSelect: (String?) -> Void

    /// - Parameter automaticTitle: nil(指定なし)を表す先頭項目の名前。読み込みでは「自動判定」、保存先では「入力に合わせる」など。
    public init(title: String, current: String?, selection: String?, automaticTitle: String = "自動判定",
                onSelect: @escaping (String?) -> Void) {
        self.automaticTitle = automaticTitle
        self.title = title
        self.current = current
        self.selection = selection
        self.onSelect = onSelect
    }

    public var body: some View {
        Menu {
            Button {
                onSelect(nil)
            } label: {
                if selection == nil {
                    Label(automaticTitle, systemImage: "checkmark")
                } else {
                    Text(automaticTitle)
                }
            }
            ForEach(TextEncodingCatalog.groups) { group in
                Divider()
                ForEach(group.options) { option in
                    Button {
                        onSelect(option.name)
                    } label: {
                        if selection == option.name {
                            Label(option.displayName, systemImage: "checkmark")
                        } else {
                            Text(option.displayName)
                        }
                    }
                }
            }
        } label: {
            // 狭いときはラベルだけを中略で縮める。macOS の Menu のラベルは Text / Image しか反映されず、
            // ViewThatFits を入れるとアイコンだけになってしまうため、単一の Text にしている。
            // 省略時は help に完全な表示を出す
            Text(label).font(.caption2).lineLimit(1).truncationMode(.middle)
        }
        .menuStyle(.borderlessButton)
        .frame(minWidth: 44, maxWidth: 230)
        .layoutPriority(0)
        .help("\(label)\n\(title)側の文字コードを選び直す")
    }

    private var label: String {
        "\(title): \(shortLabel)"
    }

    /// 見出し(「左:」など)を省いた短いラベル
    private var shortLabel: String {
        if let selection { return "\(TextEncodingCatalog.displayName(for: selection))(指定)" }
        if let current { return current }
        return automaticTitle
    }
}
