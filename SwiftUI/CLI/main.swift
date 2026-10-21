// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: diffloupe CLIのエントリポイント。SIGINT/SIGTERMを比較タスクのキャンセルへ
// マッピングし、CLIRunnerの終了コードをFoundation.exitで返す(0=一致/1=差異/2=引数エラー/3=処理エラー)。

import Foundation
import DiffLoupeCLI

// SIGINT/SIGTERMを無視し(デフォルト終了を抑止)、DispatchSourceでキャンセルへ変換する
signal(SIGINT, SIG_IGN)
signal(SIGTERM, SIG_IGN)

let cancellation = CancellationToken()
let signalQueue = DispatchQueue(label: "diffloupe.signal")
// トップレベル変数としてプロセス終了まで生存させるため、明示的な保持は不要
let signalSources: [DispatchSourceSignal] = [SIGINT, SIGTERM].map { signal in
    let source = DispatchSource.makeSignalSource(signal: signal, queue: signalQueue)
    source.setEventHandler { cancellation.cancel() }
    source.resume()
    return source
}
_ = signalSources

// トップレベルawait(SE-0343)でCLIRunnerを実行し、終了コードを返す
let code = await CLIRunner.run(
    arguments: Array(CommandLine.arguments.dropFirst()),
    cancellation: cancellation
)
Foundation.exit(code)
