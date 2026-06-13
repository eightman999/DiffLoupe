// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: 比較処理を実行するバックグラウンドスレッドクラスの宣言

//
// Created by eightman on 25/07/13.
//

#ifndef COMPAREWORKER_H
#define COMPAREWORKER_H

#include <QThread>
#include <QString>
#include <vector>
#include "core/diffresult.h"
#include "core/comparator.h"

namespace DiffLoupe {

    /**
     * @brief フォルダ比較をバックグラウンドで実行するワーカースレッド
     */
    class CompareWorker : public QThread {
        Q_OBJECT

    public:
        /**
         * @brief コンストラクタ
         * @param parent 親オブジェクト
         */
        explicit CompareWorker(QObject *parent = nullptr);

        /** デストラクタ */
        ~CompareWorker();

        /** フォルダパスを設定 */
        void setFolders(const QString &folderA, const QString &folderB);

        /** 比較設定を指定 */
        void setSettings(const Comparator::Settings &settings);

        signals:
            /** 比較処理が完了したときに送出 */
            void comparisonFinished(const std::vector<DiffResult> &results);

        /** 進捗メッセージ更新 */
        void progressUpdated(const QString &message);
        /** 進捗率更新 */
        void progressPercentage(int percentage);

        /** エラー発生通知 */
        void errorOccurred(const QString &error);

    protected:
        /** スレッド本体 */
        void run() override;

    private:
        QString m_folderA;                 //!< フォルダAパス
        QString m_folderB;                 //!< フォルダBパス
        Comparator::Settings m_settings;   //!< 比較設定
    };

} // namespace DiffLoupe

#endif // COMPAREWORKER_H
