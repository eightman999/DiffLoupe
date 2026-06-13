// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: バイナリデータを16進数で表示し差分をハイライトするウィジェット

#include "hexviewer.h"
#include <QByteArray>

namespace DiffLoupe {

HexViewer::HexViewer(QWidget *parent) : QWidget(parent) {
    setupUI();
}

HexViewer::~HexViewer() = default;

void HexViewer::setupUI() {
    QVBoxLayout *mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);

    // 単一ファイル表示用のエディタ
    m_singleTextEdit = new QTextEdit(this);
    m_singleTextEdit->setReadOnly(true);
    m_singleTextEdit->setFontFamily("Consolas");
    m_singleTextEdit->setFontPointSize(10);

    // 左右差分表示用のスプリッター
    m_splitter = new QSplitter(Qt::Horizontal, this);
    m_splitter->setVisible(false);

    // 左側のエディタ（ファイルA）
    m_leftTextEdit = new QTextEdit(this);
    m_leftTextEdit->setReadOnly(true);
    m_leftTextEdit->setFontFamily("Consolas");
    m_leftTextEdit->setFontPointSize(10);

    // 右側のエディタ（ファイルB）
    m_rightTextEdit = new QTextEdit(this);
    m_rightTextEdit->setReadOnly(true);
    m_rightTextEdit->setFontFamily("Consolas");
    m_rightTextEdit->setFontPointSize(10);

    // スプリッターに追加
    m_splitter->addWidget(m_leftTextEdit);
    m_splitter->addWidget(m_rightTextEdit);
    m_splitter->setSizes({50, 50});

    // メインレイアウトに追加
    mainLayout->addWidget(m_singleTextEdit);
    mainLayout->addWidget(m_splitter);
    setLayout(mainLayout);

    // 同期スクロールの設定
    connect(m_leftTextEdit->verticalScrollBar(), &QScrollBar::valueChanged,
            this, &HexViewer::syncScroll1);
    connect(m_rightTextEdit->verticalScrollBar(), &QScrollBar::valueChanged,
            this, &HexViewer::syncScroll2);
}

void HexViewer::setFile(const QString &file) {
    m_filePathA = file;
    m_filePathB.clear();
    m_isDiffMode = false;
    
    // 単一ファイル表示モードに切り替え
    m_singleTextEdit->setVisible(true);
    m_splitter->setVisible(false);
    
    updateSingleView();
}

void HexViewer::setFiles(const QString &fileA, const QString &fileB) {
    m_filePathA = fileA;
    m_filePathB = fileB;
    m_isDiffMode = true;
    
    // 左右差分表示モードに切り替え
    m_singleTextEdit->setVisible(false);
    m_splitter->setVisible(true);
    
    // ファイルを読み込み
    QFile file1(m_filePathA);
    if (file1.open(QIODevice::ReadOnly)) {
        m_dataA = file1.readAll();
    } else {
        m_dataA.clear();
    }

    QFile file2(m_filePathB);
    if (file2.open(QIODevice::ReadOnly)) {
        m_dataB = file2.readAll();
    } else {
        m_dataB.clear();
    }

    performComparison();
    updateDiffViews();
}

void HexViewer::clear() {
    m_singleTextEdit->clear();
    m_leftTextEdit->clear();
    m_rightTextEdit->clear();
    m_filePathA.clear();
    m_filePathB.clear();
    m_dataA.clear();
    m_dataB.clear();
    m_diffs.clear();
    m_isDiffMode = false;
    
    // 単一ファイル表示モードに戻す
    m_singleTextEdit->setVisible(true);
    m_splitter->setVisible(false);
}

void HexViewer::updateSingleView() {
    m_singleTextEdit->clear();

    if (m_filePathA.isEmpty()) {
        return;
    }

    QFile file(m_filePathA);
    if (!file.open(QIODevice::ReadOnly)) {
        m_singleTextEdit->setText("ファイルを開けません: " + m_filePathA);
        return;
    }

    QByteArray data = file.readAll();
    QString hexDump;
    int bytesPerLine = 16;

    for (int i = 0; i < data.size(); i += bytesPerLine) {
        hexDump += formatHexLine(i, data.mid(i, bytesPerLine));
        hexDump += "\n";
    }
    m_singleTextEdit->setText(hexDump);
}

void HexViewer::performComparison() {
    m_diffs.clear();
    
    if (m_dataA.isEmpty() && m_dataB.isEmpty()) {
        return;
    }

    // シンプルな線形比較を実装
    int lenA = m_dataA.size();
    int lenB = m_dataB.size();
    int commonLen = qMin(lenA, lenB);

    for (int i = 0; i < commonLen; ++i) {
        if (m_dataA[i] != m_dataB[i]) {
            // 変更箇所として記録
            m_diffs.append({DiffType::Modified, i, 1});
        }
    }

    // サイズが異なる場合の処理
    if (lenA > lenB) {
        m_diffs.append({DiffType::Deleted, commonLen, lenA - commonLen});
    } else if (lenB > lenA) {
        m_diffs.append({DiffType::Inserted, commonLen, lenB - commonLen});
    }
}

void HexViewer::updateDiffViews() {
    m_leftTextEdit->clear();
    m_rightTextEdit->clear();

    if (m_dataA.isEmpty() && m_dataB.isEmpty()) {
        return;
    }

    // 左側の表示を生成
    QString leftHex = generateHexDisplay(m_dataA, true);
    QString rightHex = generateHexDisplay(m_dataB, false);

    m_leftTextEdit->setHtml(leftHex);
    m_rightTextEdit->setHtml(rightHex);
}

QString HexViewer::generateHexDisplay(const QByteArray& data, bool isLeft) {
    QString html = "<pre style='font-family: Consolas; font-size: 10pt;'>";
    int bytesPerLine = 16;
    
    for (int i = 0; i < data.size(); i += bytesPerLine) {
        QString line = formatHexLine(i, data.mid(i, bytesPerLine));
        
        // 差分箇所のハイライト
        bool hasHighlight = false;
        for (const auto& diff : m_diffs) {
            if (i >= diff.start && i < diff.start + diff.length) {
                QColor color;
                if (diff.type == DiffType::Modified) {
                    color = QColor(255, 255, 0, 100); // 黄色
                } else if (diff.type == DiffType::Inserted && !isLeft) {
                    color = QColor(0, 255, 0, 100); // 緑色
                } else if (diff.type == DiffType::Deleted && isLeft) {
                    color = QColor(255, 0, 0, 100); // 赤色
                }
                
                if (color.isValid()) {
                    line = QString("<span style='background-color: %1;'>%2</span>")
                           .arg(color.name())
                           .arg(line);
                    hasHighlight = true;
                }
            }
        }
        
        html += line + "\n";
    }
    
    html += "</pre>";
    return html;
}

QString HexViewer::formatHexLine(int offset, const QByteArray& lineData, int highlightStart, int highlightLen, const QColor& color) {
    QString line;
    int bytesPerLine = 16;
    
    // オフセット
    line += QString("%1 ").arg(offset, 8, 16, QChar('0')).toUpper();
    
    // Hex部分
    for (int j = 0; j < bytesPerLine; ++j) {
        if (j < lineData.size()) {
            line += QString("%1 ").arg(static_cast<quint8>(lineData[j]), 2, 16, QChar('0')).toUpper();
        } else {
            line += "   ";
        }
        if (j == bytesPerLine / 2 - 1) {
            line += " "; // 中央にスペース
        }
    }
    line += " ";
    
    // ASCII部分
    for (int j = 0; j < bytesPerLine; ++j) {
        if (j < lineData.size()) {
            char c = lineData[j];
            if (c >= 32 && c <= 126) { // 表示可能なASCII文字
                line += c;
            } else {
                line += ".";
            }
        }
    }
    
    return line;
}

void HexViewer::syncScroll1(int value) {
    if (m_syncingScroll) return;
    m_syncingScroll = true;
    m_rightTextEdit->verticalScrollBar()->setValue(value);
    m_syncingScroll = false;
}

void HexViewer::syncScroll2(int value) {
    if (m_syncingScroll) return;
    m_syncingScroll = true;
    m_leftTextEdit->verticalScrollBar()->setValue(value);
    m_syncingScroll = false;
}

} // namespace DiffLoupe
