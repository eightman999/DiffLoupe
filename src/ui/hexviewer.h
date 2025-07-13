#ifndef HEXVIEWER_H
#define HEXVIEWER_H

#include <QWidget>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QFile>

namespace DiffLoupe {

class HexViewer : public QWidget {
    Q_OBJECT

public:
    explicit HexViewer(QWidget *parent = nullptr);
    ~HexViewer();

    void setFile(const QString &file);
    void clear();

private:
    QTextEdit *m_textEdit;
    QString m_filePath;

    void updateHexView();
};

} // namespace DiffLoupe

#endif // HEXVIEWER_H
