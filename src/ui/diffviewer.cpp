#include "diffviewer.h"
#include "workers/fileloadworker.h"
#include <algorithm>

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
    startLoading();
}

void DiffViewer::setEncoding(const QString &encoding) {
    m_encoding = encoding;
    startLoading();
}

void DiffViewer::clear() {
    m_textEdit->clear();
    m_fileA.clear();
    m_fileB.clear();
    m_contentA.clear();
    m_contentB.clear();
    if (m_workerA) {
        m_workerA->quit();
        m_workerA->wait();
        delete m_workerA;
        m_workerA = nullptr;
    }
    if (m_workerB) {
        m_workerB->quit();
        m_workerB->wait();
        delete m_workerB;
        m_workerB = nullptr;
    }
}

void DiffViewer::startLoading() {
    m_textEdit->clear();

    if (m_fileA.isEmpty() && m_fileB.isEmpty()) {
        return;
    }



    if (m_workerA) { m_workerA->quit(); m_workerA->wait(); delete m_workerA; }
    if (m_workerB) { m_workerB->quit(); m_workerB->wait(); delete m_workerB; }
    m_workerA = new FileLoadWorker(this);
    m_workerB = new FileLoadWorker(this);
    m_workerA->setFilePath(m_fileA);
    m_workerB->setFilePath(m_fileB);
    m_workerA->setEncoding(m_encoding);
    m_workerB->setEncoding(m_encoding);
    connect(m_workerA, &FileLoadWorker::fileLoaded, this, [this](const QString &c){ m_contentA = c; updateDiffView(); });
    connect(m_workerB, &FileLoadWorker::fileLoaded, this, [this](const QString &c){ m_contentB = c; updateDiffView(); });
    m_workerA->start();
    m_workerB->start();
}

void DiffViewer::updateDiffView() {
    if ((m_fileA.isEmpty() || !m_workerA || !m_workerA->isRunning()) &&
        (m_fileB.isEmpty() || !m_workerB || !m_workerB->isRunning())) {
        QStringList linesA = m_contentA.split('\n');
        QStringList linesB = m_contentB.split('\n');
        int maxLines = std::max(linesA.size(), linesB.size());
        QString html = "<table style='font-family:monospace;width:100%'>";
        for (int i=0;i<maxLines;i++) {
            QString a = i < linesA.size()? linesA[i] : "";
            QString b = i < linesB.size()? linesB[i] : "";
            QString style = (a==b)?"":"background-color:#ffd0d0;";
            html += QString("<tr style='%1'><td style='white-space:pre'>%2</td><td style='white-space:pre'>%3</td></tr>")
                    .arg(style)
                    .arg(a.toHtmlEscaped())
                    .arg(b.toHtmlEscaped());
        }
        html += "</table>";
        m_textEdit->setHtml(html);
    }
}

} // namespace DiffLoupe
