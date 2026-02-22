# 扉（Door）建設オブジェクト実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `door-plan-2026-02-22` |
| ステータス | `Draft` |
| 作成日 | `2026-02-22` |
| 最終更新日 | `2026-02-22` |
| 作成者 | `Claude` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: 壁で囲まれた空間への出入りを制御する手段がない。現状は壁に穴を開けるか、壁を完全に閉じるかの二択しかない。
- 到達したい状態: 壁の一部に扉を設置し、魂が自動的に開閉して通過できる。プレイヤーはロック/アンロックで通行を制御できる。
- 成功指標:
  - 扉が壁の一部として設置・建設できる
  - 魂が扉を自動で開閉して通過できる
  - パスファインディングが扉の開閉コストを考慮する
  - プレイヤーがロック/アンロックを切り替えられる

## 2. スコープ

### 対象（In Scope）

- `BuildingType::Door` の追加
- 扉コンポーネントと状態管理（Open / Closed / Locked）
- 扉の設置（新規設置 + 既存壁の置換）
- 設置条件の検証（左右 or 上下に壁が必要）
- シンプルな1フェーズ建設プロセス（素材: Wood×1 + Bone×1）
- パスファインディングへの扉コスト統合（閉じた扉 = 待機時間から自動算出されるコスト）
- 壁接続スプライトシステムとの連携（扉を壁として接続扱い）
- 開/閉の2枚スプライト切替
- 魂の自動開閉メカニズム
- プレイヤーによるロック/アンロックUI
- ロックされた扉は完全に障害物として扱う

### 非対象（Out of Scope）

- 開閉アニメーション（回転・スライド等）— スプライト切替のみ
- 扉の耐久度・破壊メカニズム
- 異なる扉の種類（鉄扉、二重扉等）
- 扉固有の音響効果
- Familiarの扉操作

## 3. 現状とギャップ

- 現状: 壁は完全な障害物として機能し、穴を開ける以外に通行手段がない
- 問題: 壁で囲った領域への出入りに柔軟性がない
- 本計画で埋めるギャップ:
  - 壁の一部を扉に置き換え、条件付き通行を可能にする
  - パスファインディングに「通行可能だがコストあり」の概念を導入する

## 4. 実装方針（高レベル）

- 方針: 既存の `BuildingType` と `Blueprint` フローを拡張し、扉を1フェーズのシンプルな建物として実装する。パスファインディングにはタイルごとの追加コスト概念を導入する。
- 設計上の前提:
  - 扉は1×1タイルの建物
  - 壁接続システムでは壁と同一視される
  - `WorldMap` に扉の位置と状態を追跡するHashMapを追加
  - 扉の開閉状態は `Door` コンポーネントと `WorldMap` の両方で管理（同期が必要）
- Bevy 0.18 APIでの注意点:
  - Observer (`Added<T>`, `Changed<T>`) を活用して扉の状態変化を検知
  - ECS Relationships は使わない（扉は独立エンティティ）

## 5. マイルストーン

## M1: 基盤 — BuildingType・コンポーネント・WorldMap拡張

### 変更内容

#### 1. `BuildingType::Door` バリアント追加

**ファイル**: `src/systems/jobs/mod.rs`
- `BuildingType` enumに `Door` を追加
- `required_materials()` に `Door => { Wood: 1, Bone: 1 }` を追加
- `BuildingType` を使用する全ての `match` を更新

#### 2. `Door` コンポーネント定義

**ファイル**: `src/systems/jobs/door.rs`（新規作成）
```rust
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Door {
    pub state: DoorState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DoorState {
    Open,
    Closed,
    Locked,
}

impl Door {
    pub fn is_passable(&self) -> bool {
        self.state != DoorState::Locked
    }
    pub fn is_open(&self) -> bool {
        self.state == DoorState::Open
    }
}
```

#### 3. `WorldMap` に扉追跡を追加

**ファイル**: `src/world/map/mod.rs`
- `WorldMap` structに `doors: HashMap<(i32, i32), Entity>` フィールドを追加
- `is_walkable()` を修正:
  - 扉タイルの場合、`obstacles[]` チェックをバイパスし `Door` コンポーネントの状態で判定
  - Locked → `false`（通行不可）
  - Open/Closed → `true`（通行可能）
- `add_door()` / `remove_door()` メソッドを追加

#### 4. 定数の追加

**ファイル**: `src/constants/building.rs`
```rust
/// 扉が開くまでの待機時間（実際のゲーム内遅延、唯一の調整パラメータ）
pub const DOOR_OPEN_DURATION_SECS: f32 = 0.5;

/// パスファインディング追加コスト（待機時間から自動算出）
/// 算出式: (待機時間 / 1タイル通過時間) * MOVE_COST_STRAIGHT
/// = (DOOR_OPEN_DURATION_SECS / (TILE_SIZE / SOUL_SPEED_BASE)) * 10
/// これにより待機時間を変更すればコストも自動追従する
pub const DOOR_OPEN_COST: i32 =
    ((DOOR_OPEN_DURATION_SECS / (TILE_SIZE / SOUL_SPEED_BASE))
        * MOVE_COST_STRAIGHT as f32) as i32;

/// 通過後に閉じるまでの遅延
pub const DOOR_CLOSE_DELAY_SECS: f32 = 1.0;
```

**設計意図**: `DOOR_OPEN_DURATION_SECS` を唯一の調整パラメータとし、パスファインディングコストを自動導出することで、A\*が予測するコストと実際の遅延時間の整合性を保証する。`SOUL_SPEED_BASE` は魂の個体差（motivation, laziness）を含まない基準速度だが、A\*のヒューリスティックも同じ基準速度を前提としているため、整合性が取れている。

### 変更ファイル
- `src/systems/jobs/mod.rs` — BuildingType拡張
- `src/systems/jobs/door.rs` — 新規: Doorコンポーネント
- `src/world/map/mod.rs` — WorldMap拡張
- `src/constants/building.rs` — 定数追加

### 完了条件
- [ ] `BuildingType::Door` が定義され、全matchが網羅されている
- [ ] `Door` コンポーネントと `DoorState` enumが定義されている
- [ ] `WorldMap` に扉の追跡機構が追加されている
- [ ] `cargo check` が成功

### 検証
- `cargo check`

---

## M2: 建設・設置 — Blueprint配置と建設完了

### 変更内容

#### 1. 扉の設置ロジック

**ファイル**: `src/interface/selection/building_place.rs`

設置バリデーション:
```
扉の設置条件:
1. 対象タイルが空（建物・stockpileなし）かつ歩行可能
2. 左右の両方が壁/扉 OR 上下の両方が壁/扉
   → (left AND right が壁/扉) OR (up AND down が壁/扉)
```

- `occupied_grids_for_building()` に `Door => vec![grid]` を追加
- `building_spawn_pos()` に `Door` のケースを追加
- `building_size()` に `Door => Vec2::splat(TILE_SIZE)` を追加
- 新規関数 `is_valid_door_placement()` を作成

#### 2. 既存壁の扉への置換

**ファイル**: `src/interface/selection/building_place.rs` または新規ファイル

壁を選択して「扉に変換」するメカニズム:
- 完成済みの壁エンティティを選択
- 左右 or 上下の壁隣接条件を確認
- 壁を削除（obstacle除去、buildings除去）
- 同じ位置に扉のBlueprintを配置
- **注意**: 壁の解体 → 扉の建設という2ステップになる

#### 3. 建設完了時の処理

**ファイル**: `src/systems/jobs/building_completion/spawn.rs`
- `Door` のスプライト（閉じた状態）とサイズを定義
- `Door` コンポーネントを `Building` エンティティに挿入

**ファイル**: `src/systems/jobs/building_completion/world_update.rs`
- 扉完了時に `world_map.doors` に登録
- 扉完了時に `world_map.obstacles` を設定（初期状態: Closed → obstacle ON）
  - ※ただし `is_walkable()` で扉チェックをobstacleより先に行うため、実質通行可能

#### 4. アセット定義

**ファイル**: `src/assets.rs`
- `door_closed: Handle<Image>` を追加
- `door_open: Handle<Image>` を追加
- アセットロード処理を追加

**画像ファイル**:
- `assets/textures/door_closed.png` — 閉じた扉スプライト
- `assets/textures/door_open.png` — 開いた扉スプライト

#### 5. UI建設メニューへの追加

扉を建設可能な建物リストに追加する。

### 変更ファイル
- `src/interface/selection/building_place.rs` — 設置ロジック
- `src/systems/jobs/building_completion/spawn.rs` — 建設完了スポーン
- `src/systems/jobs/building_completion/world_update.rs` — WorldMap更新
- `src/assets.rs` — アセット定義
- `assets/textures/door_closed.png` — 新規画像
- `assets/textures/door_open.png` — 新規画像
- UI関連ファイル（建設メニュー）

### 完了条件
- [ ] 扉を壁の隣に設置できる（左右 or 上下に壁が必要）
- [ ] 設置条件を満たさない場所ではゴーストが赤くなる
- [ ] 素材（Wood×1, Bone×1）が運ばれ、建設作業で完成する
- [ ] 完成した扉に `Door` コンポーネントが付いている
- [ ] `WorldMap.doors` に登録される
- [ ] `cargo check` が成功

### 検証
- `cargo check`
- 手動: 壁の隣に扉を配置 → 建設完了を確認

---

## M3: パスファインディング — 扉コスト統合

### 変更内容

#### 1. パスファインディングに扉コスト追加

**ファイル**: `src/world/pathfinding.rs`

現在のコスト計算:
```rust
let move_cost = if is_diagonal { MOVE_COST_DIAGONAL } else { MOVE_COST_STRAIGHT };
let tentative_g = current_g + move_cost;
```

変更後:
```rust
let base_cost = if is_diagonal { MOVE_COST_DIAGONAL } else { MOVE_COST_STRAIGHT };
let door_cost = world_map.get_door_cost(nx, ny); // 0 or DOOR_OPEN_COST
let tentative_g = current_g + base_cost + door_cost;
```

- `WorldMap` に `get_door_cost(x, y) -> i32` メソッドを追加:
  - 扉なし → 0
  - 開いている扉 → 0
  - 閉じている扉 → `DOOR_OPEN_COST`（`DOOR_OPEN_DURATION_SECS` から自動算出、約9）
  - ロックされた扉 → 到達不可（`is_walkable` で弾かれるので呼ばれない）

- `find_path()`, `find_path_to_adjacent()`, `find_path_to_boundary()` の3関数全てを更新

#### 2. WorldMap への参照渡し

パスファインディング関数に `WorldMap` 参照が必要。現在の `PathfindingContext` のシグネチャを確認し、`WorldMap` が既に渡されていなければ追加する。

### 変更ファイル
- `src/world/pathfinding.rs` — コスト計算変更
- `src/world/map/mod.rs` — `get_door_cost()` メソッド追加

### 完了条件
- [ ] 閉じた扉を通るルートに追加コスト（`DOOR_OPEN_COST`、待機時間から自動算出）が適用される
- [ ] 開いた扉は追加コストなし
- [ ] ロックされた扉は完全に通行不可
- [ ] 迂回路が短い場合は扉を避けるルートが選択される
- [ ] `cargo check` が成功

### 検証
- `cargo check`
- 手動: 扉と迂回路がある場面で、魂が適切なルートを選ぶか確認

---

## M4: 開閉メカニズム — 自動開閉とスプライト切替

### 変更内容

#### 1. 自動開閉システム

**ファイル**: `src/systems/jobs/door.rs`（M1で作成したファイルに追加）

```
door_auto_open_system:
  - 全ての閉じた扉（DoorState::Closed）を走査
  - 扉の位置から半径1タイル以内に魂がいるか確認
  - 魂のパス（目的地）が扉タイルを通過するか確認
  - 条件を満たせば DoorState::Open に遷移
  - スプライトを door_open に切替
  - WorldMap の obstacle を解除

door_auto_close_system:
  - 全ての開いた扉（DoorState::Open）を走査
  - 扉タイル上および隣接タイルに魂がいないか確認
  - いなければ閉じるタイマーを開始（DOOR_CLOSE_DELAY_SECS）
  - タイマー完了時に DoorState::Closed に遷移
  - スプライトを door_closed に切替
  - WorldMap の obstacle を設定
```

#### 2. 扉通過時の魂の動作

**ファイル**: `src/entities/damned_soul/movement/pathfinding.rs`

魂が扉タイルに到達した時:
- 扉が Closed → 一時停止（`DOOR_OPEN_DURATION_SECS` = 0.5秒）→ 扉が Open になるのを待つ → 通過
- 扉が Open → そのまま通過
- 扉が Locked → パスが無効化され、再計算される

#### 3. DoorCloseTimer コンポーネント

```rust
#[derive(Component)]
pub struct DoorCloseTimer {
    pub timer: Timer,
}
```

扉が開いた後、周囲に魂がいなくなった時点でタイマー開始。タイマー完了で閉じる。

#### 4. WorldMap 同期

扉の状態変更時に `WorldMap` を同期:
- Open → `obstacles[idx] = false`
- Closed → `obstacles[idx] = true`
- Locked → `obstacles[idx] = true`

### 変更ファイル
- `src/systems/jobs/door.rs` — 開閉システム
- `src/entities/damned_soul/movement/pathfinding.rs` — 扉到着時の待機処理
- `src/systems/jobs/mod.rs` — システム登録

### 完了条件
- [ ] 魂が閉じた扉に近づくと自動で開く
- [ ] 魂が通過後、一定時間で自動的に閉じる
- [ ] スプライトが開/閉で切り替わる
- [ ] 扉の上に魂がいる間は閉じない
- [ ] WorldMapのobstacle状態が扉の状態と同期している
- [ ] `cargo check` が成功

### 検証
- `cargo check`
- 手動: 魂が扉を通過する一連の流れを確認

---

## M5: 壁接続 — スプライト連携

### 変更内容

#### 1. 壁接続システムで扉を壁として認識

**ファイル**: `src/systems/visual/wall_connection.rs`

- `is_wall()` 関数（もしくは同等のチェック）を修正:
  - `building.kind == BuildingType::Wall` のチェックに `|| building.kind == BuildingType::Door` を追加
- 扉が設置/削除された時に隣接する壁の接続スプライトを更新するトリガーを追加

#### 2. 扉自体のスプライト

扉は壁接続システムの16バリエーションを使わない。扉自体は常に `door_open` または `door_closed` スプライトを表示する。ただし、隣接する壁は扉方向に「接続あり」として壁スプライトを選択する。

### 変更ファイル
- `src/systems/visual/wall_connection.rs` — 扉を壁接続に含める

### 完了条件
- [ ] 扉の両隣の壁が正しい接続スプライト（端ではなく接続）を表示する
- [ ] 扉を設置/削除した時に隣接壁のスプライトが更新される
- [ ] `cargo check` が成功

### 検証
- `cargo check`
- 手動: 壁-扉-壁 の並びでスプライトが正しく接続されるか確認

---

## M6: ロック/アンロックUI

### 変更内容

#### 1. コンテキストメニューにロック/アンロック追加

**ファイル**: `src/interface/ui/components.rs`
- `MenuAction` enumに `ToggleDoorLock(Entity)` を追加

**ファイル**: `src/interface/ui/panels/context_menu.rs`
- 扉を右クリック（またはコンテキストメニュー表示）時に「ロック」/「アンロック」ボタンを表示
- 現在の状態に応じてラベルを切替:
  - Locked → 「アンロック」
  - Open/Closed → 「ロック」

**ファイル**: `src/interface/ui/interaction/menu_actions.rs`
- `ToggleDoorLock` ハンドラーを追加:
  - Locked → Closed に遷移（アンロック）
  - Open/Closed → Locked に遷移（ロック）
  - `WorldMap` のobstacle状態を同期
  - スプライトを更新（ロック時は閉じた扉スプライト、将来的にはロック専用スプライトも可）

#### 2. ロック状態の視覚的フィードバック

ロックされた扉は閉じた扉と同じスプライトを使用する（初期実装）。
将来的にロック専用のスプライトを追加可能。

### 変更ファイル
- `src/interface/ui/components.rs` — MenuAction拡張
- `src/interface/ui/panels/context_menu.rs` — メニュー項目追加
- `src/interface/ui/interaction/menu_actions.rs` — アクションハンドラー

### 完了条件
- [ ] 扉のコンテキストメニューに「ロック」/「アンロック」が表示される
- [ ] ロック操作で扉が Locked 状態になる
- [ ] ロック状態の扉を魂が通過できない（障害物扱い）
- [ ] アンロック操作で Closed 状態に戻る
- [ ] パスファインディングがロック状態を正しく反映する
- [ ] `cargo check` が成功

### 検証
- `cargo check`
- 手動: ロック/アンロックの切替と魂の経路変化を確認

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| パスファインディングのコスト追加が全体パフォーマンスに影響 | 中 | `get_door_cost()` はHashMap lookup 1回のみ。扉が少なければ影響軽微。パフォーマンス計測で確認 |
| 扉の開閉状態とWorldMapの同期漏れ | 高 | 状態変更を1箇所のヘルパー関数に集約し、Door コンポーネントと WorldMap を同時に更新する |
| 壁接続スプライトの更新漏れ | 低 | 既存の壁接続システムのトリガー機構を活用。`Added<Building>` オブザーバーで自動更新 |
| 魂が扉の前で詰まる | 中 | 扉到着時の待機ロジックにタイムアウトを設け、長時間待機した場合は再パスファインディング |
| 既存壁→扉変換時のデータ不整合 | 中 | 壁削除と扉Blueprint設置を1フレーム内で処理し、中間状態を最小化 |

## 7. 検証計画

- 必須:
  - `cargo check` — 各マイルストーン完了時
- 手動確認シナリオ:
  1. **設置テスト**: 壁-壁の間に扉を設置 → 建設完了
  2. **自動開閉テスト**: 魂が扉に向かって歩き、開いて通過し、閉じるまで
  3. **パスファインディングテスト**: 扉を通るルートと迂回ルートの選択
  4. **ロックテスト**: ロック → 魂が迂回 → アンロック → 魂が扉を通る
  5. **壁接続テスト**: 壁-扉-壁の配置でスプライトが正しく表示される
  6. **エッジケース**: 扉上で魂が停止 → 扉が閉じないこと。複数魂が同時に通過。
- パフォーマンス確認:
  - 多数の扉（20+）がある場面でのパスファインディング性能

## 8. ロールバック方針

- どの単位で戻せるか: マイルストーン単位。各Mは独立してcommit可能。
- 戻す時の手順:
  - M1-M2: `BuildingType::Door` 関連コードを削除し、全matchから除去
  - M3: パスファインディングのコスト計算を元に戻す
  - M4-M6: 各システムファイルを削除/revert

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M6全て未着手

### 次のAIが最初にやること

1. この計画書を読む
2. M1から順に実装を開始する
3. 各マイルストーンの完了条件を確認してから次へ進む

### ブロッカー/注意点

- 扉のスプライト画像が必要（`door_open.png`, `door_closed.png`）— 画像生成ワークフローで作成
- パスファインディング関数のシグネチャ変更が必要な場合、呼び出し元全てを更新すること
- `is_walkable()` の修正は慎重に — 全てのゲームロジックに影響する

### 参照必須ファイル

- `docs/building.md` — 建設システム仕様
- `src/systems/jobs/mod.rs` — BuildingType定義
- `src/world/pathfinding.rs` — パスファインディング
- `src/world/map/mod.rs` — WorldMap
- `src/systems/visual/wall_connection.rs` — 壁接続
- `src/interface/selection/building_place.rs` — 設置ロジック
- `src/systems/jobs/building_completion/` — 建設完了処理

### 最終確認ログ

- 最終 `cargo check`: 未実施
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M6の全マイルストーンが完了
- [ ] `docs/building.md` に扉の仕様が追記されている
- [ ] `cargo check` が成功
- [ ] 手動テストで全シナリオが通過

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-22` | `Claude` | 初版作成 |
