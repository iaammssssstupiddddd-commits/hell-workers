# タスク実行システムの規約

タスクの追加や変更を行う際は、以下のルールを遵守し、型安全性とデータの整合性を担保すること。

## 1. AssignedTask のデータ構造化
タスクpayloadは `crates/hw_jobs/src/tasks/` のfeature別ファイルに専用structとして定義し、`crates/hw_jobs/src/tasks/mod.rs` の `AssignedTask` に `Variant(VariantData)` 形式で追加すること。

**理由**: フィールド名の明示により引数の誤渡しを防止し、コードの可読性を高めるため。

## 2. クエリの集約（TaskQueries）
Soul実行側のクエリは `crates/hw_soul_ai/src/soul_ai/execute/task_execution/context/queries.rs`、Familiar検索・割当側のクエリは `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/context.rs` に集約すること。

**理由**: 使い魔と魂の間で検索条件やアクセスするコンポーネントの整合性を強制し、一方の変更による他方の不整合を防止するため。

## 3. 実行コンテキストの利用
魂の個別AI処理を実装する際は、`TaskExecutionContext` を通じてデータ（タスク状態、パス、インベントリ等）にアクセスすること。

**理由**: システム関数の引数を最小限に保ち、共通処理（パス更新、中断処理等）の共通化を容易にするため。
