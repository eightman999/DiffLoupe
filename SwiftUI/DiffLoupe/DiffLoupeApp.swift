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
            CommandMenu("レポート") {
                Button("HTMLを保存…") { model.saveReport(as: .html) }
                    .disabled(model.result == nil)
                Button("CSVを保存…") { model.saveReport(as: .csv) }
                    .disabled(model.result == nil)
                Button("JSONを保存…") { model.saveReport(as: .json) }
                    .disabled(model.result == nil)
                Button("Markdownを保存…") { model.saveReport(as: .markdown) }
                    .disabled(model.result == nil)
            }
            CommandMenu("コピー計画") {
                Button("左 → 右の計画を表示…") {
                    model.prepareSyncPlan(direction: .leftToRight)
                }
                .disabled(model.result == nil)
                Button("右 → 左の計画を表示…") {
                    model.prepareSyncPlan(direction: .rightToLeft)
                }
                .disabled(model.result == nil)
            }
            CommandMenu("3-way差分") {
                Button("3つのファイルを比較…") {
                    model.showThreeWayComparison()
                }
            }
        }

        Settings {
            SettingsView()
                .environmentObject(model)
#if PRO
                .environmentObject(licenseManager)
#endif
        }
    }
}
