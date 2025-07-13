#include "diffviewer.h"

namespace DiffLoupe {

DiffViewer::DiffViewer(QWidget *parent) : QWidget(parent) {
    m_textEdit = new QTextEdit(this);
    m_textEdit->setReadOnly(true);

    QVBoxLayout *layout = new QVBoxLayout(this);
    layout->addWidget(m_textEdit);
    setLayout(layout);
}

DiffViewer::~DiffViewer() = default;

void DiffViewer::setFiles(const QString &fileA, const QString &fileB) {
    m_fileA = fileA;
    m_fileB = fileB;
    updateDiffView();
}

void DiffViewer::setEncoding(const QString &encoding) {
    m_encoding = encoding;
    updateDiffView();
}

void DiffViewer::clear() {
    m_textEdit->clear();
    m_fileA.clear();
    m_fileB.clear();
}

QString DiffViewer::readFileContent(const QString &filePath, QTextCodec *codec) {
    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return QString();
    }

    QTextStream in(&file);
    if (codec) {
        in.setEncoding(static_cast<QStringConverter::Encoding>(codec->mibEnum()));
    }
    return in.readAll();
}

void DiffViewer::updateDiffView() {
    m_textEdit->clear();

    if (m_fileA.isEmpty() && m_fileB.isEmpty()) {
        return;
    }

    QTextCodec *codec = nullptr;
    if (m_encoding == "Auto-detect") {
        // TODO: 自動検出ロジックを実装
        codec = QTextCodec::codecForName("UTF-8"); // 仮
    } else {
        codec = QTextCodec::codecForName(m_encoding.toUtf8());
    }

    QString contentA;
    if (!m_fileA.isEmpty()) {
        contentA = readFileContent(m_fileA, codec);
    }
    QString contentB;
    if (!m_fileB.isEmpty()) {
        contentB = readFileContent(m_fileB, codec);
    }

    // TODO: 差分アルゴリズムを実装し、HTMLで整形してm_textEditにセット
    // 現時点では単純にファイル内容を表示
    m_textEdit->setHtml(
        QString("<pre>") +
        QString("<b>--- File A ---</b><br>") + contentA.toHtmlEscaped() + QString("<br><br>") +
        QString("<b>--- File B ---</b><br>") + contentB.toHtmlEscaped() +
        QString("</pre>")
    );
}

} // namespace DiffLoupe
