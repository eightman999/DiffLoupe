// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: FileLoadWorkerクラス宣言。指定ファイルを非同期に読み込む

#ifndef FILELOADWORKER_H
#define FILELOADWORKER_H

#include <QThread>
#include <QString>

namespace DiffLoupe {

    /**
     * @brief ファイル読み込みを非同期で行うワーカー
     */
    class FileLoadWorker : public QThread {
        Q_OBJECT

    public:
        /**
         * @brief コンストラクタ
         * @param parent 親オブジェクト
         */
        explicit FileLoadWorker(QObject *parent = nullptr);

        /** デストラクタ */
        ~FileLoadWorker();

        /** 読み込むファイルパスを設定 */
        void setFilePath(const QString &filePath);

        /** エンコーディングを設定 */
        void setEncoding(const QString &encoding);

        signals:
            /** ファイル読み込み完了 */
            void fileLoaded(const QString &content);
        /** エラー通知 */
        void errorOccurred(const QString &error);

    protected:
        /** スレッド実行 */
        void run() override;

    private:
        QString m_filePath;   //!< 読み込み対象ファイル
        QString m_encoding;   //!< 使用するエンコーディング
    };

} // namespace DiffLoupe

#endif // FILELOADWORKER_H