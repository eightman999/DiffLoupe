#ifndef DIFFVIEWER_H
#define DIFFVIEWER_H

#include <QWidget>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QFile>
#include <QTextStream>
#include <QTextCodec>
#include "workers/fileloadworker.h"

namespace DiffLoupe {

class DiffViewer : public QWidget {
    Q_OBJECT

public:
    explicit DiffViewer(QWidget *parent = nullptr);
    ~DiffViewer();

    void setFiles(const QString &fileA, const QString &fileB);
    void setEncoding(const QString &encoding);
    void clear();

private:
    QTextEdit *m_textEdit;
    QString m_fileA;
    QString m_fileB;
    QString m_encoding;
    QString m_contentA;
    QString m_contentB;
    FileLoadWorker *m_workerA = nullptr;
    FileLoadWorker *m_workerB = nullptr;

    void startLoading();
    void updateDiffView();
};

} // namespace DiffLoupe

#endif // DIFFVIEWER_H
