# Crate Boundaries & Core Logic Separation

本ドキュメントは、プロジェクトの Cargo Workspace におけるクレートの境界、型の配置、および Bevy システム（System）の分離に関する開発規則です。

## 0. クレートの命名規則

*   ドメインロジックをカプセル化する新しいクレートを作成する場合は、必ず `hw_<domain_name>` のプレフィックスをつけること（例: `hw_core`, `hw_jobs`, `hw_visual`）。
*   `bevy_app` は App Shell として唯一プレフィックスを持たない Root クレートとする。
*   単なる `utils` や `components` といった、特定のドメインを持たない雑多な共通クレートの新設は禁止する。

## 1. クレート境界の基本原則 (Crate Boundaries)

*   **Leaf Crates (`hw_*`) は「Pure Domain (純粋ロジック層)」である**
    *   各ドメイン特有の Data Model (Component / Resource)、振る舞い (純粋な関数・System)、ヘルパー関数を所有する。
    *   **Bevy型システムへの依存は許容・推奨される。** (`Entity`, `Query`, `Res`, `Component` 等を自由に使用してよい)
    *   **Root クレート (`bevy_app`) への逆依存は完全禁止**（Cargoの循環依存制約によるコンパイルエラーを防ぐため）。

*   **Root (`bevy_app`) は「App Shell (インフラ・配線層)」である**
    *   各 Leaf クレートが定義した System や Plugin を繋ぎ合わせる「配線」と、全体のアセット (`GameAssets`) 管理、ウィンドウ・レンダリングなどの「ガワ」に徹する。
    *   純粋なビジネスロジックやAIの意思決定アルゴリズムなどを `bevy_app` に新規実装してはならない。
    *   Leaf クレート側でアセット情報などが必要な場合は、`bevy_app` 固有の型を直接要求せず、専用のリソース（例: `WallVisualHandles`）やトレイト（例: `UiAssets`）を定義し、`bevy_app` 側から注入（Inject）する。

*   **UI クレート (`hw_ui`) の特殊な位置づけ (プレゼンテーション層)**
    *   `hw_ui` は純粋なドメインロジックではなく、プレゼンテーション層として機能する。
    *   `hw_core` などの型を参照しつつ、自らの UI 状態（例: `AreaEditSession`, `Selection` 等）を管理する。
    *   フォントや画像などのアセットに依存する必要があるため、直接 `GameAssets` を参照するのではなく、`UiAssets` トレイトなどを通じて `bevy_app` から注入（Inject）される設計を維持すること。

## 2. 型定義と所有権（Ownership）のルール

複数のシステム間でデータをやり取りするための型（struct や enum）や、関数の戻り値（Result / Outcome 型）は、**「その処理の主たる責務を持つ Leaf クレート (`hw_*`)」** 側で定義し、Root 側がそれを `use` して利用する。

*   **パターン A (基盤型):** `hw_core` へ配置（例: `PlayMode`, `ResourceType`, 共有 Event / Relationship）。
*   **パターン B (ドメイン特化型):** 該当するドメインクレート (`hw_jobs`, `hw_familiar_ai` 等) へ配置（例: `FamiliarStateDecisionResult`）。
*   **パターン C (App Shell固有状態):** `bevy_app` に残留するが、判定基準を厳格に適用する。
    *   **【判定基準】**
        *   **Leafに移すべき型:** 複数の Leaf crate が読み書き・参照する必要がある型。
        *   **残留が許容される型:** `bevy_app` のシステム引数・UI表示など「ガワ」の用途でしか使わない型（例: `GameAssets`）。

## 3. コアロジックと ECS の分離・連携原則 (Core Logic & ECS Integration)

Bevy固有のAPI（`Query`, `Commands`, `Res`）をどのように扱うかは、処理のフェーズ（意思決定か、実行か）およびパフォーマンスの観点から以下のルールに従う。

### 3.1. 意思決定フェーズ (Decision / Planning)

「どのタスクを割り当てるか」「次にどの状態に遷移するか」「どこへ移動するか」といった複雑な計算ロジックは、可能な限り**副作用を持たない純粋なロジック**として切り出すことを推奨する。

*   **実装:** `Commands` や `mut Query` を内部で使わず、読み取り専用の `Query` (または `SystemParam`) と必要なデータを引数に取り、結果を独自の `Outcome` 型や `Message` で返す設計にする。
*   **エラーハンドリング:** 処理の失敗（パス探索失敗、エンティティ消失等）は、必ず `Result<Outcome, Error>` として返すこと。関数内でパニックさせたりエラーを握りつぶしたりしない。

#### パフォーマンス例外の適用基準

「ホットパスではデータ詰め替えを避けてよい（純粋関数化をスキップしてよい）」という例外は、「毎フレーム全エンティティ・全タイルをスキャンする描画更新や物理演算レベルの処理」にのみ適用を許可する。AI の意思決定においては純粋関数化を原則とする。

### 3.2. 実行・反映フェーズ (Execution / Apply)

決定された `Outcome` に基づいて実際にゲームの世界を変更する処理（エンティティの Spawn/Despawn、Relationship の更新、コンポーネントの `mut` 変更）は、**Leaf クレート (`hw_*`) 内の `System` または `Observer` として直接実装してよい。**

*   **エラーの処理 (System側):** アダプターとなる System は、意思決定フェーズから返された `Error` を受け取り適切に処理する。一時的な失敗であれば `warn!` 等でログ出力し、永続的な失敗（ゲーム状態の更新が必要）であれば、対象エンティティに `TaskFailed` コンポーネントを付与する等のリカバリー処理を `Commands` で行うこと。
*   **他クレートへの書き込み:** `Cargo.toml` の依存関係が許す範囲（例: `hw_soul_ai` が `hw_jobs` に依存している状態）であれば、Leaf クレート内の System が他クレートの型を `mut Query` で操作したり、関連する Entity を `Commands` で Despawn することは正当なアーキテクチャとして認める。
*   **異なる Leaf クレート間の連携 (Pub/Sub パターン):** 異なるドメイン（例: `hw_jobs` でのタスク完了と、`hw_visual` でのエフェクト再生）を連携させる場合、システムを直接呼び出すような密結合を避けること。共通のイベント（`hw_core` に定義）を発行し、他方のクレートが `Observer` やシステムでそれを購読（Subscribe）する Pub/Sub パターンを採用する。新規実装では必ず適用し、既存コードの改修時も改修範囲内でミラーコンポーネントパターン（`hw_core::visual_mirror::*`）への切り替えを検討すること。
*   **Observer:** 即時反応を要する `Observer` ハンドラーも同様に、関連する Leaf クレート内で直接 `Commands` を伴って実装してよい。これらを無理に Root のアダプターへ移譲する必要はない。

### 3.3. Plugin 登録の責務

システムや Observer を Bevy アプリケーションに登録する責務は以下の通り分割する。

*   **Leaf 側の責務 (`hw_*`):**
    *   **「自クレート内で完結する」システム** や Observer は、自クレートが提供する `Plugin` の中で `add_systems` / `add_observer` を行い、適切な `GameSystemSet` に属させる。
    *   **【「完結」の定義】**: `Cargo.toml` の `dependencies` に含まれるクレートの型のみを使用するシステムは「完結している」と見なし、Leaf の `Plugin` で登録する。
*   **Root 側の責務 (`bevy_app`):**
    *   各 Leaf クレートの `Plugin` を束ねて `App` に追加する。
    *   `GameSystemSet` 全体の実行順序（Input -> Logic -> Visual 等）を定義する。
    *   `GameAssets` 等の **`bevy_app` 固有の型を必要とするシステム** に限り、Root 側で登録を行う。
    *   **Ordering Facade (例外):** `bevy_app` 固有型への依存がないシステムであっても、他システムとの厳密な ordering を root の scheduling facade で一元管理する必要がある場合は、Leaf が実装を所有しつつ Root 側で `add_systems` / `add_observer` を行ってよい。ただし「登録元は一箇所だけ」の原則は維持すること（Leaf Plugin と Root の二重登録は禁止）。
*   **実装所有と登録元の分離原則:**
    *   **実装本体の所有先（どのクレートに関数があるか）** は、そのシステムが依存する型の所在で決まる。root-only 依存がなければ Leaf crate に置く。
    *   **登録元（`add_systems` を呼ぶ場所）** は、上記 Leaf / Root の責務区分に従い選択する。
    *   この 2 軸を分けて考えることで、「実装は Leaf に、登録は Root の ordering facade から」というパターンを矛盾なく扱える。
