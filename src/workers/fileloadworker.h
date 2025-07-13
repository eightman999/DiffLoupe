#ifndef FILELOADWORKER_H
#define FILELOADWORKER_H

#include <QThread>
#include <QString>

namespace DiffLoupe {

    class FileLoadWorker : public QThread {
        Q_OBJECT

    public:
        explicit FileLoadWorker(QObject *parent = nullptr);
        ~FileLoadWorker();

        void setFilePath(const QString &filePath);
        void setEncoding(const QString &encoding);

        signals:
            void fileLoaded(const QString &content);
        void errorOccurred(const QString &error);

    protected:
        void run() override;

    private:
        QString m_filePath;
        QString m_encoding;
    };

} // namespace DiffLoupe

#endif // FILELOADWORKER_H