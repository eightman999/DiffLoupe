#include "hexviewer.h"
#include <QByteArray>

namespace DiffLoupe {

HexViewer::HexViewer(QWidget *parent) : QWidget(parent) {
    m_textEdit = new QTextEdit(this);
    m_textEdit->setReadOnly(true);
    m_textEdit->setFontFamily("Monospace");
    m_textEdit->setFontPointSize(10);

    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->addWidget(m_textEdit);
    setLayout(layout);
}

HexViewer::~HexViewer() = default;

void HexViewer::setFile(const QString &file) {
    m_filePath = file;
    updateHexView();
}

void HexViewer::clear() {
    m_textEdit->clear();
    m_filePath.clear();
}

void HexViewer::updateHexView() {
    m_textEdit->clear();

    if (m_filePath.isEmpty()) {
        return;
    }

    QFile file(m_filePath);
    if (!file.open(QIODevice::ReadOnly)) {
        m_textEdit->setText("ファイルを開けません: " + m_filePath);
        return;
    }

    QByteArray data = file.readAll();
    QString hexDump;
    int bytesPerLine = 16;

    for (int i = 0; i < data.size(); i += bytesPerLine) {
        hexDump += QString("%1 ").arg(i, 8, 16, QChar('0')).toUpper(); // オフセット

        // Hex部分
        for (int j = 0; j < bytesPerLine; ++j) {
            if (i + j < data.size()) {
                hexDump += QString("%1 ").arg(static_cast<quint8>(data[i + j]), 2, 16, QChar('0')).toUpper();
            } else {
                hexDump += "   ";
            }
            if (j == bytesPerLine / 2 - 1) {
                hexDump += " "; // 中央にスペース
            }
        }
        hexDump += " ";

        // ASCII部分
        for (int j = 0; j < bytesPerLine; ++j) {
            if (i + j < data.size()) {
                char c = data[i + j];
                if (c >= 32 && c <= 126) { // 表示可能なASCII文字
                    hexDump += c;
                } else {
                    hexDump += ".";
                }
            }
        }
        hexDump += "\n";
    }
    m_textEdit->setText(hexDump);
}

} // namespace DiffLoupe
