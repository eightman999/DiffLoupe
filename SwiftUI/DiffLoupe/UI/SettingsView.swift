// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 設定画面。既定除外・ユーザー除外glob・内容比較の正規化・リネーム検出の各設定を切り替える。
// 設定キーはAppModelに集約し、UserDefaults/AppStorageで永続化する(次回比較から反映)

import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

public struct SettingsView: View {
    @EnvironmentObject private var model: AppModel
    @AppStorage(AppModel.recursiveComparisonKey) private var recursiveComparison = true
    @AppStorage(AppModel.useDefaultExcludesKey) private var useDefaultExcludes = true
    @AppStorage(AppModel.userExcludePatternsKey) private var excludePatternsRaw = ""
    @AppStorage(AppModel.userIncludePatternsKey) private var includePatternsRaw = ""
    @AppStorage(AppModel.filterPresetsKey) private var filterPresetsRaw = ""
    @AppStorage(AppModel.ignoreLineEndingsKey) private var ignoreLineEndings = false
    @AppStorage(AppModel.ignoreTrailingWhitespaceKey) private var ignoreTrailingWhitespace = false
    @AppStorage(AppModel.normalizeWhitespaceKey) private var normalizeWhitespace = false
    @AppStorage(AppModel.ignoreCaseKey) private var ignoreCase = false
    @AppStorage(AppModel.structuredModeKey) private var structuredModeRaw = StructuredComparisonMode.disabled.rawValue
    @AppStorage(AppModel.detectRenamesKey) private var detectRenames = false
    @AppStorage(AppModel.autoRefreshKey) private var autoRefreshEnabled = false

    public init() {}

    public var body: some View {
        // 項目が多く(エディションの追加セクションを含めると)画面の高さを超えるため、スクロール可能にして高さを抑える
        ScrollView {
            // ScrollView内では幅が確定しないため、折り返しが窓幅に収まるよう明示する
            settingsForm
                .frame(width: 440)
                .padding(20)
        }
        .frame(width: 480, height: 640)
    }

    private var settingsForm: some View {
        Form {
            Section("エディション") {
                HStack {
                    Text("現在のエディション")
                    Spacer()
                    Text(model.edition.name)
                        .fontWeight(.semibold)
                        .foregroundColor(model.edition.maxFolderEntries == nil ? .green : .secondary)
                }
                if let limit = model.edition.maxFolderEntries {
                    Text("\(model.edition.name)版はフォルダ比較が\(limit)ファイルまでに制限されます。上限なしで比較できるのは有償のPro版です。")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }

            if let section = model.edition.settingsSection(model: model) {
                section
            }

            Section("比較") {
                Toggle("サブフォルダを含める", isOn: $recursiveComparison)
                Text("無効にすると、選択した左右フォルダの直下だけを比較します。次回の比較から反映されます。")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Toggle("既定の除外パターンを有効にする", isOn: $useDefaultExcludes)
                Text(FolderComparator.Options.defaultExcludedNames.sorted().joined(separator: "、") + " を比較対象から除外します。次回の比較から反映されます。")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Toggle("リネーム/移動候補を検出する", isOn: $detectRenames)
                Text("左右の片側のみにあるファイル同士を内容ハッシュで照合し、リネーム/移動の候補を表示します。有効にすると追加のハッシュ計算が入るため比較時間が長くなることがあります。")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Section("内容比較の正規化") {
                Toggle("改行コードの違いを無視する", isOn: $ignoreLineEndings)
                Toggle("行末の空白を無視する", isOn: $ignoreTrailingWhitespace)
                Toggle("連続する空白を1つに正規化する", isOn: $normalizeWhitespace)
                Toggle("大文字と小文字の違いを無視する", isOn: $ignoreCase)
                Picker("構造化比較", selection: $structuredModeRaw) {
                    ForEach(StructuredComparisonMode.allCases, id: \.rawValue) { mode in
                        Text(mode.displayName).tag(mode.rawValue)
                    }
                }
                Text("すべてオフの場合はバイト単位の厳密比較になります(既定)。正規化設定はフォルダ比較と詳細差分ビューの両方に反映されます。")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Text("構造化比較はJSON・XML・CSV/TSVの表記揺れ(キー順・属性順・引用・区切りなど)を正規化して比較します。パースできないファイルは元のテキストで比較します。")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Section("ユーザー除外パターン") {
                TextEditor(text: $excludePatternsRaw)
                    .font(.system(.body, design: .monospaced))
                    .frame(minHeight: 110)
                    .overlay(
                        RoundedRectangle(cornerRadius: 4)
                            .stroke(Color.secondary.opacity(0.3))
                    )
                Text("1行に1パターン。\(excludePatternsHint)")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Text("有効なパターン: \(normalizedPatterns.isEmpty ? "なし" : normalizedPatterns.joined(separator: "、"))")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Section("ユーザーインクルードパターン") {
                TextEditor(text: $includePatternsRaw)
                    .font(.system(.body, design: .monospaced))
                    .frame(minHeight: 90)
                    .overlay(
                        RoundedRectangle(cornerRadius: 4)
                            .stroke(Color.secondary.opacity(0.3))
                    )
                Text("1行に1パターン。\(excludePatternsHint)")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Text("非空のとき、ファイル名または相対パスが一致するファイルだけを比較対象に含めます。空のときは従来どおり全ファイルが対象です(除外は常に優先されます)。")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Text("有効なパターン: \(normalizedIncludePatterns.isEmpty ? "なし" : normalizedIncludePatterns.joined(separator: "、"))")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Section("自動再比較") {
                Toggle("フォルダ変更時に自動再比較", isOn: $autoRefreshEnabled)
                    .onChange(of: autoRefreshEnabled) { enabled in
                        model.setAutoRefreshEnabled(enabled)
                    }
                Text("左右のフォルダのファイル追加・削除・サイズ/更新時刻の変更を監視し、変更を検知すると自動で再比較します。監視は既定で1秒間隔です。比較中に変更を検知した場合は、その比較が終わった後に一度だけ再比較します。既定では無効です。")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Section("フィルタプリセット (opt-in)") {
                ForEach(FilterPreset.all) { preset in
                    Toggle(preset.displayName, isOn: presetBinding(preset.id))
                }
                Text("選択したプリセットの除外パターンが次回の比較から追加されます。既定ではすべて無効です。")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
    }

    private var excludePatternsHint: String {
        "* はセパレータをまたがない任意列、** はセパレータをまたぐ任意列、? は任意の1文字です。"
    }

    private var normalizedPatterns: [String] {
        AppModel.normalizeExcludePatterns(excludePatternsRaw)
    }

    private var normalizedIncludePatterns: [String] {
        AppModel.normalizeIncludePatterns(includePatternsRaw)
    }

    /// プリセット選択の永続化形式(カンマ区切りID)とSet<String>の橋渡し。
    private var selectedPresetIDs: Set<String> {
        Set(
            filterPresetsRaw
                .split(separator: ",")
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
        )
    }

    private func presetBinding(_ id: String) -> Binding<Bool> {
        Binding(
            get: { selectedPresetIDs.contains(id) },
            set: { isOn in
                var ids = selectedPresetIDs
                if isOn { ids.insert(id) } else { ids.remove(id) }
                filterPresetsRaw = ids.sorted().joined(separator: ",")
            }
        )
    }
}
