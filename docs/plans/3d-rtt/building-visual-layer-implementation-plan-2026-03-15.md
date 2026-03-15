# 建築物ビジュアル多層レイヤー実装計画（フェーズA・B）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `building-visual-layer-implementation-plan-2026-03-15` |
| ステータス | `Done` |
| 作成日 | `2026-03-15` |
| 最終更新日 | `2026-03-15` |
| 作成者 | `Gemini CLI` |
| 関連提案 | `docs/proposals/3d-rtt/related/building-visual-layer-plan-2026-03-12.md` |
| ロードマップ対応 | `MS-Pre-B`（`docs/plans/3d-rtt/milestone-roadmap.md`） |
| 関連Issue/PR | N/A |

> **ブラッシュアップ履歴**: 2026-03-15 (Claude Sonnet 4.6) — 実コード調査に基づき以下を追記。
> - 現行 Z 値の実態（全建築物が Z_AURA を継承している問題）を M1 スコープに明示
> - Z 定数テーブルを既存定数との対比表に更新
> - `VisualLayerKind` 型定義スケッチ追加
> - M2 子エンティティスポーンパターン（`with_children`）追加
> - M3 の `wall_connection` / `tank` / `mud_mixer` ごとのクエリ変更パターン追加

## 1. 目的

- 解決したい課題:
  - 建築物が1エンティティ=1スプライトに固定されており、床・壁・配線などの重層的な表現ができない。
  - Z座標の管理がマジックナンバー化しており、保守性が低い。
- 到達したい状態:
  - 建築物が親子構造（Building -> VisualLayers）を持ち、複数の視覚要素を独立して制御できる。
  - Z座標が構造化された定数（Zスロット）で管理されている。
- 成功指標（MS-Pre-B 完了条件）:
  - 既存の建築物の見た目を維持したまま、内部構造が親子階層にリファクタリングされていること。
  - 建築物ルートから `Sprite` を外しても既存 visual system（`wall_connection`, `tank` 等）が破綻しないこと。
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功すること。

## 2. スコープ

### 対象（In Scope）

- `render.rs` への `Z_BUILDING_*` 定数群の追加。
- `spawn_completed_building` の親子構造へのリファクタリング。
- `VisualLayerKind` コンポーネントの導入（`hw_visual` クレート）。
- 建築物ルートの `Sprite` を直接クエリしている既存システム（`hw_visual::wall_connection`, `hw_visual::tank` 等）の VisualLayer 子エンティティ参照への追従。

### 非対象（Out of Scope）

- 3D-RTT インフラの構築（将来フェーズ）。
- 配線・配管の具体的なロジック実装（フェーズC以降）。

## 3. 現状とギャップ

### 現状コード確認（実調査済み）

**`spawn_completed_building` の現状（`spawn.rs`）**

```rust
// Blueprint の Transform をそのまま渡すため、Z 値は Z_AURA(0.2) を継承
let building_entity = commands.spawn((
    Building { kind: bp.kind, is_provisional },
    Sprite { image: sprite_image, custom_size: Some(custom_size), ..default() },
    *transform,  // ← Z = Z_AURA(0.2) がそのまま入る
    ...
)).id();
```

**Blueprint のスポーン（`placement.rs`）**

```rust
// 全ての建築物 Blueprint が Z_AURA で生成される
Transform::from_xyz(geometry.draw_pos.x, geometry.draw_pos.y, Z_AURA)
```

**`wall_connection.rs` の現状**

```rust
// WorldMap からエンティティを引いて、そのエンティティの Sprite を直接取得
mut q_sprites: Query<&mut Sprite>,
// ...
if let Ok(mut sprite) = q_sprites.get_mut(entity) {
    update_wall_sprite(entity, gx, gy, &mut sprite, ...);
}
```

**`tank.rs` の現状**

```rust
// Building エンティティから直接 Sprite を取得
mut q_tanks: Query<(&Building, &Stockpile, Option<&StoredItems>, &mut Sprite), With<Building>>,
```

### ギャップ整理

| ギャップ | 影響 | 担当マイルストーン |
| --- | --- | --- |
| 全建築物の Z が `Z_AURA(0.2)` に固定 | 床が壁より上に来るなど重なり順が崩れる | M1 |
| Building ルートに Sprite が直結 | 複数レイヤーの差し込みができない | M2 |
| `wall_connection` / `tank` 等が Building の Sprite を直接クエリ | M2 後に参照が壊れる | M3 |

## 4. 実装方針（高レベル）

- 方針: 「データとしての建築物（親）」と「表示としてのレイヤー（子）」を ECS 階層で分離する。
- 設計上の前提: 2D スプライトのまま進め、Z 軸の僅かな差（0.01、必要なら 0.001）で前後関係を制御する。
  - f32 の精度は 0.1 付近で約 1.2e-8 なので、0.001 刻みへの切り下げはいつでも可能。
- Bevy 0.18 APIでの注意点: `Sprite` コンポーネントの初期化、親子関係（`with_children`）の構築。
- `BuildingBounceEffect` の配置: **親エンティティに残す**。`building_bounce_animation_system` は `Transform.scale` を操作し、Bevy が親 scale を子に伝播するため、子への移動は不要。
- `spawn_completed_building` の戻り値: M2 後も **親エンティティの Entity を返す**。呼び出し元が `ProvisionalWall` / `Door` を `.insert()` する既存の構造を維持する。
- `VisualLayerKind` の配置クレート: 既存の `hw_visual` クレートに新規モジュール (`src/layer/mod.rs`) として追加する。`Cargo.toml` への crate 追加は不要。

### `VisualLayerKind` 型定義スケッチ

```rust
// crates/hw_visual/src/layer/mod.rs（新規）
use bevy::prelude::*;

/// 建築物ビジュアルレイヤーの種別。
/// 親 Building エンティティの子として生成され、Sprite を保持する。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualLayerKind {
    /// 床・地面面（Z_BUILDING_FLOOR = 0.05）
    Floor,
    /// 壁・構造体（Z_BUILDING_STRUCT = 0.12）
    Struct,
    /// 装飾レイヤー（Z_BUILDING_DECO = 0.15）
    Deco,
    /// 照明・エフェクト（Z_BUILDING_LIGHT = 0.18）
    Light,
}
```

### Z 定数テーブル（既存値との対比）

| 定数名 | 値 | 区分 |
| --- | --- | --- |
| `Z_MAP` | 0.00 | 既存 |
| `Z_MAP_SAND` | 0.01 | 既存 |
| `Z_MAP_DIRT` | 0.02 | 既存 |
| `Z_MAP_GRASS` | 0.03 | 既存 |
| **`Z_BUILDING_FLOOR`** | **0.05** | **新規** |
| `Z_ROOM_OVERLAY` | 0.08 | 既存 |
| `Z_ITEM` | 0.10 | 既存 |
| **`Z_BUILDING_STRUCT`** | **0.12** | **新規** |
| **`Z_BUILDING_DECO`** | **0.15** | **新規** |
| **`Z_BUILDING_LIGHT`** | **0.18** | **新規** |
| `Z_AURA` | 0.20 | 既存（Blueprint Z はここを使用） |
| `Z_CHARACTER` | 1.00 | 既存 |

## 5. マイルストーン

## M1: フェーズA - Z定数の構造化と移行

- 変更内容: `render.rs` への `Z_BUILDING_*` 定数群の追加と、`spawn.rs` での BuildingType 別 Z 値の割り当て。
- **重要**: 現在 `spawn.rs` は Blueprint の `Transform` をそのまま転用しているため、すべての建築物の Z が `Z_AURA(0.2)` になっている。M1 でこの問題も同時に解消する。
- Z定数の設計値（ロードマップ MS-Pre-B 準拠）:
  - `Z_BUILDING_FLOOR = 0.05`（床・地面面: アイテムより下）
  - `Z_BUILDING_STRUCT = 0.12`（壁・構造体: アイテムより上、Z_AURA より下）
  - `Z_BUILDING_DECO = 0.15`（装飾レイヤー）
  - `Z_BUILDING_LIGHT = 0.18`（照明・エフェクトレイヤー: Z_AURA=0.2 より下）
- BuildingType 別 Z 割り当て方針:
  - `Floor` → `Z_BUILDING_FLOOR`
  - `Wall` / `Door` / `Bridge` → `Z_BUILDING_STRUCT`
  - `Tank` / `MudMixer` / `RestArea` / `WheelbarrowParking` → `Z_BUILDING_STRUCT`
  - `SandPile` / `BonePile` → `Z_BUILDING_FLOOR`（地面上の堆積物）
- 変更ファイル:
  - `crates/hw_core/src/constants/render.rs`
  - `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`（`*transform` をそのまま使わず、Z だけ上書き）
- 完了条件:
  - [ ] 全ての建築物スポーン箇所が `Z_BUILDING_*` 定数を参照している。
  - [ ] 建築物の Z が `Z_AURA(0.2)` から正しい値に変わっている。
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - 手動目視確認（壁・床・タンクの重なり順）

## M2: フェーズB - VisualLayerKind 子エンティティ化

- 変更内容: `Building` エンティティの子として `VisualLayerKind` エンティティを生成し、そこに `Sprite` を移譲する。
- 変更ファイル:
  - `crates/hw_visual/src/layer/mod.rs` (新規モジュール、hw_visual クレート内に追加)
  - `crates/hw_visual/src/lib.rs` (layer モジュールの pub 宣言追加)
  - `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- 完了条件:
  - [ ] 建築完了時に親子階層が構築されている。
  - [ ] `spawn_completed_building` が引き続き親エンティティの Entity を返す。
  - [ ] `BuildingBounceEffect` は親エンティティに残り、子の Sprite にスケールが伝播することを確認。
  - [ ] 既存のバウンスアニメーションが正しく動作する（`cargo check` 後に手動目視確認）。
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - 手動目視確認

### spawn.rs 変更パターン（スケッチ）

```rust
// 変更後の spawn_completed_building（抜粋）
let z = match bp.kind {
    BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => Z_BUILDING_FLOOR,
    _ => Z_BUILDING_STRUCT,
};
let parent_transform = Transform::from_xyz(transform.translation.x, transform.translation.y, z);
let layer_kind = match bp.kind {
    BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => VisualLayerKind::Floor,
    _ => VisualLayerKind::Struct,
};

let building_entity = commands
    .spawn((
        Building { kind: bp.kind, is_provisional },
        parent_transform,
        Name::new(format!("Building ({:?})", bp.kind)),
        BuildingBounceEffect { ... },
    ))
    .with_children(|parent| {
        parent.spawn((
            layer_kind,
            Sprite { image: sprite_image, custom_size: Some(custom_size), ..default() },
            Transform::default(), // ローカル座標 Z=0。グローバル Z は親の Z がそのまま使われる
            Name::new(format!("VisualLayer ({:?})", layer_kind)),
        ));
    })
    .id();
```

> **注意**: 子エンティティの `Transform` はローカル座標（親相対）なので Z = 0 でよい。最終的な描画 Z は親の GlobalTransform から決まる。

## M3: フェーズC - 既存 visual system の VisualLayerKind 追従

- 変更内容: 建築物ルートの `Sprite` を直接参照している既存システムを、VisualLayerKind 子エンティティ経由に切り替える。
- **M3 着手前に `grep -rn "Query.*Sprite\|Sprite.*Query" crates/hw_visual/src/` 等で全洗い出しを必ず実施すること。**
- 対象システム（現時点での調査結果）:
  - `hw_visual::wall_connection`（壁接続バリアント切替）
  - `hw_visual::tank`（タンク満タン表示）
  - `hw_visual::mud_mixer`（`Query<(Entity, &mut Sprite), With<MudMixerStorage>>`）— 要確認
- 変更ファイル:
  - `crates/hw_visual/src/wall_connection.rs`
  - `crates/hw_visual/src/tank.rs`
  - `crates/hw_visual/src/mud_mixer.rs`（要確認）
- 完了条件:
  - [ ] 上記システムが Building 親エンティティの `Sprite` を直接クエリしていない。
  - [ ] 建築物ルートから `Sprite` コンポーネントを除去しても visual system が破綻しない。
  - [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` 通過。
  - [ ] 壁接続バリアント・タンク表示が手動目視確認で正常。
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - 手動目視確認（壁接続・タンク）

### wall_connection.rs の変更パターン

`wall_connection.rs` は WorldMap からエンティティを引いてから Sprite を取得する構造なので、単純な Query フィルタ変更では済まない。

```rust
// 変更前
mut q_sprites: Query<&mut Sprite>,
// ...
if let Ok(mut sprite) = q_sprites.get_mut(entity) {  // entity = Building エンティティ
    update_wall_sprite(..., &mut sprite, ...);
}

// 変更後（Children を使って VisualLayerKind::Struct の子を探す）
q_children: Query<&Children>,
q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
// ...
if let Ok(children) = q_children.get(entity) {
    for &child in children.iter() {
        if let Ok((layer_kind, mut sprite)) = q_visual_layers.get_mut(child) {
            if *layer_kind == VisualLayerKind::Struct {
                update_wall_sprite(entity, gx, gy, &mut sprite, ...);
                break;
            }
        }
    }
}
```

### tank.rs の変更パターン

```rust
// 変更前
mut q_tanks: Query<(&Building, &Stockpile, Option<&StoredItems>, &mut Sprite), With<Building>>,

// 変更後（Building から子への間接参照）
q_tanks: Query<(Entity, &Building, &Stockpile, Option<&StoredItems>)>,
q_children: Query<&Children>,
q_visual_layers: Query<(&VisualLayerKind, &mut Sprite)>,
// ...
for (entity, building, stockpile, stored_items_opt) in q_tanks.iter() {
    if building.kind != BuildingType::Tank { continue; }
    if let Ok(children) = q_children.get(entity) {
        for &child in children.iter() {
            if let Ok((layer_kind, mut sprite)) = q_visual_layers.get_mut(child) {
                if *layer_kind == VisualLayerKind::Struct {
                    sprite.image = /* ... */;
                    break;
                }
            }
        }
    }
}
```

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Zソートの逆転 | 壁がアイテムの下に隠れる等 | `Z_BUILDING_FLOOR(0.05)` 〜 `Z_BUILDING_LIGHT(0.18)` の範囲を `Z_ITEM(0.1)` / `Z_AURA(0.2)` との大小関係で設計済み。フェーズC以降でレイヤー数が増えた場合は 0.001 刻みに切り下げて対応する。 |
| アニメーションの消失 | バウンスしなくなる | `BuildingBounceEffect` は親エンティティに残す（`building_bounce_animation_system` が操作する `Transform.scale` は Bevy 経由で子に伝播されるため、変更不要）。 |
| M3 調査漏れ | 見た目が壊れる箇所が残る | M3 着手前に `Sprite` を直接クエリしている全 visual system を `grep` で洗い出してから実装する。 |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
- 手動確認シナリオ:
  - 建築完了時に、エディタ（Bevy Inspector 等）で親子関係が構築されていることを確認。
  - 壁、床、タンクそれぞれの重なり順に異常がないか確認。
  - 壁接続バリアント（孤立・T字・十字等）が正しく切り替わることを確認。

## 8. ロールバック方針

- どの単位で戻せるか: 各マイルストーン単位。
- 戻す時の手順: Git revert または親子構造を解消し、親エンティティに直接 Sprite を戻す。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%` (実装完了)
- 完了済みマイルストーン: M1, M2, M3
- 未着手/進行中: なし

### 次のAIが最初にやること

**M1（Z定数の構造化）**

1. `crates/hw_core/src/constants/render.rs` に以下の Z 定数群を追加する:
   ```rust
   pub const Z_BUILDING_FLOOR: f32 = 0.05;   // 床・地面面
   pub const Z_BUILDING_STRUCT: f32 = 0.12;  // 壁・構造体
   pub const Z_BUILDING_DECO: f32 = 0.15;    // 装飾レイヤー
   pub const Z_BUILDING_LIGHT: f32 = 0.18;   // 照明・エフェクト
   ```
2. `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs` を修正する:
   - `*transform` をそのまま使っている箇所を、BuildingType 別に Z 定数を割り当てるよう変更する。
   - 具体的には `match bp.kind` で `z` 変数を決め、`Transform::from_xyz(transform.translation.x, transform.translation.y, z)` を使う。
   - 現状: 全建築物が Blueprint から継承した `Z_AURA(0.2)` を持っている — これが最初に修正すべき問題。

**M2（子エンティティ化）**

3. `crates/hw_visual/src/layer/mod.rs` を新規作成し `VisualLayerKind` を定義する（セクション4のスケッチ参照）。
4. `crates/hw_visual/src/lib.rs` に `pub mod layer;` を追加する。
5. `spawn.rs` で `Building` の子に `VisualLayerKind` エンティティを生成する（セクション M2 のスケッチ参照）。

**M3（visual system 追従）**

6. `grep -rn "Query.*Sprite\|mut Sprite\|&mut Sprite" crates/hw_visual/src/` で全対象を洗い出す。
7. `wall_connection.rs`、`tank.rs`、`mud_mixer.rs` を `Children` + `VisualLayerKind` 経由に切り替える（セクション M3 のスケッチ参照）。

### ブロッカー/注意点

- **⚠️ 現行 Z 問題**: 全建築物の Z が `Z_AURA(0.2)` になっている（`placement.rs` の Blueprint スポーン時の Z をそのまま継承するため）。M1 で必ず修正すること。
- Z 定数の範囲: `Z_BUILDING_FLOOR(0.05)` 〜 `Z_BUILDING_LIGHT(0.18)`。`Z_ITEM(0.1)` と `Z_AURA(0.2)` の間に設計する。
- `BuildingBounceEffect` は親エンティティに残したまま実装すること（子への移動は不要）。
- `spawn_completed_building` の戻り値（親 Entity）を変えないこと。呼び出し元の `ProvisionalWall` / `Door` の `.insert()` が依存している。
- `VisualLayerKind` は `hw_visual` クレートに追加する（新規クレートではないため `Cargo.toml` のワークスペース変更は不要）。
- `wall_connection.rs` の Sprite 取得は WorldMap 経由のエンティティ参照を経由しているため、単純な Query フィルタ変更では済まない（セクション M3 のパターン参照）。
- M3 着手前に全 visual system の Sprite 直接参照を `grep` で必ず洗い出すこと。

### 参照必須ファイル

- `crates/hw_core/src/constants/render.rs`
- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- `crates/bevy_app/src/interface/selection/building_place/placement.rs`（Blueprint スポーン Z の現状確認）
- `crates/hw_visual/src/wall_connection.rs`
- `crates/hw_visual/src/tank.rs`
- `crates/hw_visual/src/mud_mixer.rs`（M3 要確認）

### Definition of Done（MS-Pre-B 完了条件）

- [ ] M1: Z定数化が完了、建築物の Z が BuildingType 別に正しく設定されている
- [ ] M2: 子エンティティ方式への移行が完了
- [ ] M3: 既存 visual system の VisualLayerKind 追従が完了
- [ ] 建築物ルートから `Sprite` を除去しても既存 visual system が破綻しない
- [ ] `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-15` | `Gemini CLI` | 初版作成。2D互換・多層レイヤー化に向けた基盤構築計画。 |
| `2026-03-15` | `Claude Sonnet 4.6` | ロードマップ(MS-Pre-B)との照合に基づき修正。コンポーネント名を `VisualLayerKind` に統一、Z定数を4値に拡張、M3（既存 visual system 追従）を追加。 |
| `2026-03-15` | `Claude Sonnet 4.6` | 実コード調査に基づくブラッシュアップ。現行 Z 値の実態（全建築物が Z_AURA=0.2 を継承）を M1 に明示。Z 定数対比表、VisualLayerKind 型スケッチ、M2 spawn パターン、M3 wall_connection/tank/mud_mixer クエリ変更パターンを追加。cargo check コマンドを CARGO_HOME プレフィックス付きに統一。 |
| `2026-03-15` | `Claude Sonnet 4.6` | MS-Pre-B 実装完了。M1/M2/M3 すべて実装し `cargo check` 通過。 |
