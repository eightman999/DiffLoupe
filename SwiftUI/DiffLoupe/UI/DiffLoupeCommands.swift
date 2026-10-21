// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: メニューバーのコマンド群(レポート・コピー計画・3-way差分)。
// 有償版は同じCommandsを使いつつ、自身の.commands {}で項目を追加する

import SwiftUI
#if canImport(DiffLoupeCore)
import DiffLoupeCore
#endif

public struct DiffLoupeCommands: Commands {
    @ObservedObject private var model: AppModel

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some Commands {
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
}
