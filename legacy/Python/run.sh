#!/bin/bash
# Copyright (c) eightman 2005-2025
# Furin-lab All rights reserved.
# 動作設計: Python版アプリケーションを起動するシェルスクリプト。環境変数を設定してmain.pyを実行


# DiffLoupe起動スクリプト
# Qtの警告メッセージを抑制

export QT_LOGGING_RULES="qt.text.font.db.warning=false;qt.qpa.fonts.warning=false"
export QT_AUTO_SCREEN_SCALE_FACTOR=1

# Pythonの実行
python main.py