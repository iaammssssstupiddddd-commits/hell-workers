# hw_world — AI Rules

このファイルは `CLAUDE.md` と `AGENTS.md` のシンボリックリンク先です。

## 責務（このクレートがやること）

- ワールドマップ（`WorldMap`）の定義：タイル・地形・壁・扉の状態管理
- 座標変換（`world_to_grid`, `grid_to_world`）
- 部屋検出システム（`room_detection`）：壁・扉・床で囲まれた空間の自動認識
- 地形生成（`mapgen`）、パス探索（`pathfinding`）（旧 `borders` / 境界スプライトは MS-3-4 で廃止）
- マップクエリユーティリティ（`find_nearest_river_grid`, `find_nearest_walkable_grid`）
- `WorldMapRead` / `WorldMapWrite` SystemParam の提供

## 禁止事項（AI がやってはいけないこと）

- **`bevy_app` への逆依存禁止**（Cargo 循環依存制約）
- **`GameAssets` / UI 型への依存を持ち込まない**
- **`hw_logistics` / `hw_soul_ai` / `hw_familiar_ai` を依存に追加しない**（これらは hw_world の下流）
- **パス探索結果をキャッシュせずに毎フレーム全タイルスキャンしない**（パフォーマンス: マップは大きい）
- **`#[allow(dead_code)]` を使用しない**

## crate 境界ルール（docs/crate-boundaries.md に基づく）

- leaf crate：Bevy 型の利用は許可
- `bevy_app` への逆依存は **完全禁止**
- 詳細: [docs/crate-boundaries.md](../../docs/crate-boundaries.md)

## 依存制約（Cargo.toml 実体）

```
# 許可
hw_core ✓
hw_jobs ✓
bevy    ✓
rand    ✓
wfc     ✓
direction ✓

# 禁止
bevy_app       ✗
hw_logistics   ✗
hw_soul_ai     ✗
hw_familiar_ai ✗
hw_ui          ✗
hw_visual      ✗
```

## 座標変換（重要）

- `WorldMap::world_to_grid(pos: Vec2) -> (i32, i32)`：ワールド座標 → グリッド座標
- `WorldMap::grid_to_world(grid: (i32, i32)) -> Vec2`：グリッド座標 → ワールド座標
- 詳細: [docs/world_layout.md](../../docs/world_layout.md)

## 部屋検出システムの ECS 契約

- `RoomBounds` Component：部屋の矩形境界
- 部屋検出は非同期で実行される（変更後の次フレーム以降に反映）
- 詳細: [docs/room_detection.md](../../docs/room_detection.md)

## docs 更新対象（変更時に必ず更新するドキュメント）

- [docs/world_layout.md](../../docs/world_layout.md)（座標・マップ仕様変更時）
- [docs/room_detection.md](../../docs/room_detection.md)（部屋検出変更時）
- `crates/hw_world/_rules.md`（このファイル）

## 検証方法

```bash
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check
# パス探索テストが存在する場合
CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo test
```

## 参照ドキュメント

- [docs/world_layout.md](../../docs/world_layout.md): マップ仕様・座標変換関数
- [docs/room_detection.md](../../docs/room_detection.md): 部屋検出システム仕様
- [docs/cargo_workspace.md](../../docs/cargo_workspace.md): crate 責務一覧
- [docs/crate-boundaries.md](../../docs/crate-boundaries.md): leaf/root 境界ルール
