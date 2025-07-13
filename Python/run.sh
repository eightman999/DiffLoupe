#!/bin/bash

# DiffLoupe起動スクリプト
# Qtの警告メッセージを抑制

export QT_LOGGING_RULES="qt.text.font.db.warning=false;qt.qpa.fonts.warning=false"
export QT_AUTO_SCREEN_SCALE_FACTOR=1

# Pythonの実行
python main.py