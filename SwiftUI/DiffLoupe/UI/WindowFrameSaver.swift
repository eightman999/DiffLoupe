// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: ウィンドウサイズ・位置を記憶するためframeAutosaveNameを設定する薄いAppKit補助

import SwiftUI
import AppKit

struct WindowFrameSaver: NSViewRepresentable {
    let autosaveName: String

    func makeNSView(context: Context) -> NSView {
        let view = FrameSaverView()
        view.autosaveName = autosaveName
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}

    private final class FrameSaverView: NSView {
        var autosaveName = ""

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard let window, !autosaveName.isEmpty else { return }
            window.setFrameAutosaveName(autosaveName)
        }
    }
}
