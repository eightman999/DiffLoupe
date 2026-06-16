// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 設定画面。既定除外パターンのオン/オフを切り替える

import SwiftUI

struct SettingsView: View {
#if PRO
    @EnvironmentObject private var licenseManager: LicenseManager
#endif
    @AppStorage(AppModel.useDefaultExcludesKey) private var useDefaultExcludes = true
#if PRO
    @State private var licenseKey = ""
    @State private var activationMessage: String?
#endif

    var body: some View {
        Form {
#if PRO
            Section("ライセンス") {
                HStack {
                    Text("現在の状態")
                    Spacer()
                    Text(licenseManager.isPro ? "Pro" : "Free")
                        .fontWeight(.semibold)
                        .foregroundColor(licenseManager.isPro ? .green : .secondary)
                }

                TextField("ライセンスキー", text: $licenseKey)
                    .textFieldStyle(.roundedBorder)

                HStack {
                    Button("制限を解除") {
                        activateLicense()
                    }
                    .disabled(licenseKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)

                    if licenseManager.isPro {
                        Button("ライセンスを削除", role: .destructive) {
                            licenseManager.clearLicense()
                            licenseKey = ""
                            activationMessage = "Freeに戻しました。"
                        }
                    }
                }

                if let activationMessage {
                    Text(activationMessage)
                        .font(.caption)
                        .foregroundColor(licenseManager.isPro ? .green : .red)
                }

                Text("Freeはフォルダ比較が\(FolderComparator.Options.freeMaxEntries)ファイルまでに制限されます。Proではこの上限を解除します。")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
#endif

            Section("比較") {
                Toggle("既定の除外パターンを有効にする", isOn: $useDefaultExcludes)
                Text(FolderComparator.Options.defaultExcludedNames.sorted().joined(separator: "、") + " を比較対象から除外します。次回の比較から反映されます。")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
        .padding(20)
        .frame(width: 480)
    }

#if PRO
    private func activateLicense() {
        if licenseManager.activate(key: licenseKey) {
            activationMessage = "Proライセンスを有効化しました。"
        } else {
            activationMessage = "ライセンスキーを確認できませんでした。"
        }
    }
#endif
}
