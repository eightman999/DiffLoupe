#include "workers/fileloadworker.h"
#include <QFile>
#include <QTextStream>
#include <QTextCodec>

namespace DiffLoupe {

    FileLoadWorker::FileLoadWorker(QObject *parent)
        : QThread(parent)
    {
    }

    FileLoadWorker::~FileLoadWorker()
    {
        if (isRunning()) {
            quit();
            wait();
        }
    }

    void FileLoadWorker::setFilePath(const QString &filePath)
    {
        m_filePath = filePath;
    }

    void FileLoadWorker::setEncoding(const QString &encoding)
    {
        m_encoding = encoding;
    }

    void FileLoadWorker::run()
    {
        try {
            QFile file(m_filePath);
            if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
                emit errorOccurred(QString("ファイルを開けません: %1").arg(m_filePath));
                return;
            }

            QTextStream stream(&file);

            // エンコーディング設定
            if (m_encoding != "Auto-detect") {
#if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
                stream.setCodec(m_encoding.toLatin1().constData());
#else
                // Qt6ではsetEncodingを使用
                if (m_encoding == "UTF-8") {
                    stream.setEncoding(QStringConverter::Utf8);
                } else if (m_encoding == "UTF-16") {
                    stream.setEncoding(QStringConverter::Utf16);
                } else if (m_encoding == "Shift_JIS") {
                    stream.setEncoding(QStringConverter::System);
                }
                // TODO: 他のエンコーディングのサポート
#endif
            }

            QString content = stream.readAll();
            emit fileLoaded(content);

        } catch (const std::exception &e) {
            emit errorOccurred(QString("エラー: %1").arg(e.what()));
        }
    }

} // namespace DiffLoupe