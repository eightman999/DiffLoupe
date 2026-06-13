# Copyright (c) eightman 2005-2025
# Furin-lab All rights reserved.
# 動作設計: PySide6アプリケーションのメインスクリプト。ログ設定後、UIを起動

import os
import sys
from PySide6.QtWidgets import QApplication
from PySide6.QtCore import QLoggingCategory
from ui_main import MainWindow

def setup_logging():
    """ログ設定と警告抑制"""
    # Qtのフォント関連の警告を抑制
    os.environ['QT_LOGGING_RULES'] = 'qt.text.font.db.warning=false'
    
    # 詳細なログ抑制設定
    QLoggingCategory.setFilterRules(
        "qt.text.font.db.debug=false\n"
        "qt.text.font.db.warning=false\n"
        "qt.qpa.fonts.debug=false\n"
        "qt.qpa.fonts.warning=false"
    )

def main():
    # ログ設定
    setup_logging()
    
    # アプリケーション初期化
    app = QApplication(sys.argv)
    
    # アプリケーション情報設定
    app.setApplicationName("ディフルーペ")
    app.setApplicationVersion("1.0.0")
    app.setOrganizationName("DiffLoupe")
    
    # メインウィンドウ表示
    window = MainWindow()
    window.show()
    
    # アプリケーション実行
    sys.exit(app.exec())

if __name__ == "__main__":
    main()
