// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: FileLoadWorkerクラス宣言。指定ファイルを非同期に読み込む

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