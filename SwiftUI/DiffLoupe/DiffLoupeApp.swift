// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: アプリのエントリポイント。メインウィンドウと設定シーンを定義する

import SwiftUI

@main
struct DiffLoupeApp: App {
#if PRO
    @StateObject private var licenseManager: LicenseManager
#endif
    @StateObject private var model: AppModel

    init() {
#if PRO
        let licenseManager = LicenseManager()
        _licenseManager = StateObject(wrappedValue: licenseManager)
        _model = StateObject(wrappedValue: AppModel(licenseManager: licenseManager))
#else
        _model = StateObject(wrappedValue: AppModel())
#endif
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(model)
#if PRO
                .environmentObject(licenseManager)
#endif
                .frame(minWidth: 900, minHeight: 540)
        }
        .defaultSize(width: 1200, height: 760)
        .commands {
            // 単一ウィンドウ運用のため「新規ウィンドウ」を無効化
            CommandGroup(replacing: .newItem) {}
        }

        Settings {
            SettingsView()
#if PRO
                .environmentObject(licenseManager)
#endif
        }
    }
}
