#ifndef DIFFVIEWER_H
#define DIFFVIEWER_H

#include <QWidget>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QFile>
#include <QTextStream>
#include <QTextCodec>

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

    void updateDiffView();
    QString readFileContent(const QString &filePath, QTextCodec *codec);
};

} // namespace DiffLoupe

#endif // DIFFVIEWER_H
