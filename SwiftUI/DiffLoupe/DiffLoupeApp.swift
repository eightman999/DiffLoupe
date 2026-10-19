// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: アプリのエントリポイント。メインウィンドウと設定シーンを定義する

import SwiftUI

@main
struct DiffLoupeApp: App {
    @StateObject private var model = AppModel(edition: FreeEdition())

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
                .frame(minWidth: 900, minHeight: 540)
        }
        .defaultSize(width: 1200, height: 760)
        .commands {
            DiffLoupeCommands(model: model)
        }

        Settings {
            SettingsView()
                .environmentObject(model)
        }
    }
}
