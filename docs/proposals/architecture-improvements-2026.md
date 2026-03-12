# アーキテクチャ改善提案 (Architecture Improvement Proposals)

現在の `hell-workers` のアーキテクチャ（Bevy 0.18, ECS Relationship/Observerの活用、4フェーズのAIシステム）は非常にモダンで堅牢に設計されています。
今後のプロジェクトのスケール（エンティティ数の増加、コードベースの拡大）を見据え、パフォーマンスと保守性の観点から以下の改善案を提案します。

---

## 提案1: Cargo Workspace によるマルチクレート化 (保守性・コンパイル速度)

**現状の課題:**
プロジェクト全体が巨大な単一のクレート (`bevy_app`) として構成されています。`crates/bevy_app/src/` 配下に `systems`, `interface`, `logistics`, `entities` など膨大なモジュールが存在します。これにより、局所的なUIの変更でもプロジェクト全体の再コンパイル（リンク処理など）が発生し、開発イテレーションが低下する傾向にあります。

**改善案:**
Cargo Workspace を導入し、ドメインごとに別クレートに分割します。
```text
hell-workers/
 ├─ Cargo.toml (Workspace root)
 ├─ crates/
 │   ├─ hw_core/      (共通のECSコンポーネント、定数、空間グリッド構造など)
 │   ├─ hw_spatial/   (Pathfinding、Spatial Gridの更新ロジック)
 │   ├─ hw_logistics/ (TransportRequest, Stockpileロジック)
 │   ├─ hw_ai/        (Soul AI, Familiar AIのロジック)
 │   ├─ hw_ui/        (Interface, UI Nodeの管理)
 │   └─ hw_visual/    (Particle, Shaders)
 └─ crates/bevy_app/src/ (main.rs と アプリケーションの組み上げ)
```

**メリット:**
- **コンパイル速度の劇的な向上**: 変更を加えたクレートとその依存先のみが再コンパイルされるため、並列コンパイルが効きやすくなります。
- **依存関係の強制**: UI が AI の内部状態に直接依存するといった意図しない結合を、Rust のモジュールシステムレベルで物理的に防ぐことができます。

---

## 提案2: AsyncComputeTaskPool を用いた非同期パスファインディング (パフォーマンス)

**現状の課題:**
ソースコード内に `PathfindingTask` をバックグラウンドスレッドに逃がす処理（AsyncComputeTaskPool）が見当たらず、パスファインディングがメインループ（またはメインループと同期するSystem実行時）に同期的に走っていると推測されます。マップが広がりワーカー数が増加した際に、パス計算がフレームレート低下（スパイク）の直接的な原因になります。

**改善案:**
Bevy の `AsyncComputeTaskPool` を活用し、経路探索を非同期化します。

```rust
// 改善イメージ
#[derive(Component)]
pub struct PathfindingTask(pub Task<Option<Path>>);

// 1. 経路が必要になったらTaskをSpawnしてコンポーネントとして付与
fn request_path_system(...) {
    let thread_pool = AsyncComputeTaskPool::get();
    let task = thread_pool.spawn(async move {
        find_path(...)
    });
    commands.entity(e).insert(PathfindingTask(task));
}

// 2. Taskの完了をポーリングするSystem
fn poll_pathfinding_system(mut commands: Commands, mut tasks: Query<(Entity, &mut PathfindingTask)>) {
    for (entity, mut task) in tasks.iter_mut() {
        if let Some(path_result) = future::block_on(future::poll_once(&mut task.0)) {
            // パス計算完了
            commands.entity(entity).remove::<PathfindingTask>().insert(path_result);
        }
    }
}
```

**メリット:**
- 何十体もの Soul が同時に遠くの搬入先を目指そうとした時でも、画面のFPSがカクつく（フレーム落ち）のを完全に防ぎます。

---

## 提案3: 局所的な A* 探索から Flow Fields (Vector Fields) への移行 (アルゴリズム・スケーラビリティ)

**現状の課題:**
タスクの割り当て時や、アイテムの搬入時に `SpatialGrid` や各種制限付きの A* (隣接探索など) で動的に最適な対象を見つけています。ワーカーが 10〜30 体程度なら問題ありませんが、物流（Logistics）の指示により何百ものエンティティが同じ目的地（特定の Stockpile や Blueprint）を目指すようになると、個別の計算負荷が跳ね上がります。

**改善案:**
主要な物流のハブ（Stockpile や巨大な建設サイト）に対する距離計算に **Flow Fields (Dijkstraマップ)** を導入します。

1. **静的キャッシュ**: マップの地形が変わったとき、または Stockpile が設置されたときに、その地点をゴールとした「全マスのゴール方向と距離」を示す距離グリッド（Flow Field）をバックグラウンドで事前計算します。
2. **O(1) ルックアップ**: 各ワーカーのタスク割り当て時や移動時に、A* を実行するのではなく「現在地の Flow Field の値」を見るだけで「次に進むべき方向」と「正確な移動コスト」が O(1) で取得できます。

**メリット:**
- 「その建築サイトに向かう」というタスクを持つワーカーが1体でも1000体でも、パスファインディングのCPU負荷が変わりません（O(1)化）。

---

## 提案4: 高頻度イベントに対する Observer と Event の使い分け最適化 (エンジンチューニング)

**現状の課題:**
アーキテクチャドキュメントには「エンティティコンポーネントの即時反応には `Observer` を使う」と定義されています。Bevy 0.14+ (0.18含む) の Observers は非常に強力で便利ですが、毎フレーム何百回も発生するようなミクロなイベント（例：アイテムを1マス運ぶたびに発生する何か、毎フレームの状態変更通知など）に対して Trigger を発行すると、コールスタックとオーバーヘッドが標準の `EventWriter`/`EventReader` よりも大きくなるケースがあります。

**改善案:**
- **Observer に適しているもの**: `OnAdd`, `OnRemove`, ワーカーのスポーン/死亡、タスクの完了通知、建築物の完成など、**ライフサイクル** に関わるイベント。
- **Event のバッチ処理に適しているもの**: 移動、微細なダメージ判定、資源の細かな放出（パーティクル生成トリガー）など、**毎フレーム大量に発生する** 可能性のあるイベント。

両者のプロファイリングを行い、ホットパス（高頻度で呼ばれる処理）の Trigger を通常の Event に置き換えることで、さらなる CPU 時間の削減が見込めます。

---

## 最後にまとめ

現在のアーキテクチャの完成度は高く、無理に全てを取り入れる必要はありません。
もし、上記の中で**「現在直面しているパフォーマンスの壁」や「開発のしやすさの課題」**に合致するものがあれば、その部分の具体的な実装手順（例: Workspace化の自動スクリプト作成や、非同期パスファインドの実装など）へと進めることが可能です。どれか深掘りしたいトピックはありますか？
