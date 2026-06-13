// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 設定画面。既定除外パターンのオン/オフを切り替える

import SwiftUI

struct SettingsView: View {
    @AppStorage(AppModel.useDefaultExcludesKey) private var useDefaultExcludes = true

    var body: some View {
        Form {
            Toggle("既定の除外パターンを有効にする", isOn: $useDefaultExcludes)
            Text(FolderComparator.Options.defaultExcludedNames.sorted().joined(separator: "、") + " を比較対象から除外します。次回の比較から反映されます。")
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .padding(20)
        .frame(width: 420)
    }
}
