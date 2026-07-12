# CPU パフォーマンス改善計画書

## メタ情報

| 項目 | 値 |
| --- | --- |
| ステータス | `Superseded` |
| 作成日 | `2026-04-16` |
| 最終再監査日 | `2026-07-12` |
| 後継計画 | `docs/plans/system-wide-runtime-performance-plan-2026-07-12.md` |

作成日: 2026-04-16
最終検証: 2026-04-16（全対象ファイルをコードで直接確認済み）

コードを直接読んで確認した問題を対象にする。計画書や提案書に既に挙げられているものは対象外。

> 2026-07-12再監査: 本計画は後継計画で置き換えた。P1/P2/P3/P5/P6/P7は実装済み。P4はproducer間共有まで実装済みだが、`CachedActiveFamiliars` / `CachedActiveYards`の毎フレーム再構築が残るため、後継計画M3Aへ移管した。本書を新規実装の正本として使わない。

## 2026-07-12 実装状況

| ID | 状況 | 後継での扱い |
| --- | --- | --- |
| P1 | 実装済み | 再実装しない |
| P2 | owner cache方式で実装済み | cleanupではなくlogical/visual Transform分離を後継M2で扱う |
| P3 | shader global time方式で実装済み | 再実装しない |
| P4 | 一部実装済み | active Familiar/Yard cacheのdirty駆動化だけを後継M3Aへ移管 |
| P5 | `FloorTileWaitingCache` / `WallTileWaitingCache`方式で実装済み | 後継M5は別基盤の`TileSiteIndex`を使ってphase/completion/curing側を扱う |
| P6 | primary Top-K allocationは実装済み | fallback用copyは意図的に維持。別candidate scratchだけを後継M3Aで扱う |
| P7 | 実装済み | 再実装しない |

---

## 問題一覧

| ID | 優先 | 内容 | 対象ファイル |
|---|---|---|---|
| P1 | 高 | Shadow Projector: 全Soul収集 + O(n log n) フルソート（毎フレーム） | `soul_shadow_projector.rs` |
| P2 | 高 | Proxy Cleanup: O(k×n) ネストループ × 4関数 | `character_proxy_3d.rs`, `visual3d.rs` |
| P3 | 中 | Task Area Material: `materials.get_mut()` 毎フレーム呼び出し → マテリアル常時 dirty | `task_area_visual.rs` + `task_area.wgsl` |
| P4 | 高 | Producer: 全Familiar/Yard Vec 毎フレーム再構築 × 16箇所 | `collect.rs` 他8ファイル |
| P5 | 中 | Floor/Wall Construction Producer: 全タイル走査 毎フレーム | `floor_construction.rs`, `wall_construction.rs` |
| P6 | 低 | Assignment Loop: `to_vec()` 2回の不要ヒープ確保 | `assignment_loop.rs` |
| P7 | 低 | DreamBubble UI Material: `material.time` 毎フレーム CPU 書き込み（world-space 版は解決済み） | `dream_bubble_material.rs` + `dream_bubble_ui.wgsl` |

---

## P1. Shadow Projector — フルソート削除

### 問題

`soul_shadow_projector.rs:39-49`

```rust
let mut projectors = q_souls
    .iter()
    .map(|transform| {
        let d = ...; (d, center)
    })
    .collect::<Vec<_>>();        // Soul 数 n の Vec 毎フレーム確保
projectors.sort_by(|a, b| a.0.total_cmp(&b.0));  // O(n log n)
```

`MAX_SOUL_SHADOW_PROJECTORS = 12`（`hw_core/src/constants/render.rs:85`）しか使わないのに全件ソート。
Soul 100体 → sort_by は約700比較。

**検証済み補足**: `sync_section_material_projectors` 等のヘルパー関数（行 98-171）は
既に値の diff チェックを内包しており、projector 配列が前フレームと同一なら GPU へは書き込まない。
真のコストは **CPU 側の Vec 確保 + O(n log n) ソートのみ**。

さらに Soul が1体も動いていないフレームでも毎回 Vec 確保 + ソートが走る。

### 改善

**Step 1**: `select_nth_unstable_by` で O(n) + O(12 log 12) に変更。

```rust
if projectors.len() > MAX_SOUL_SHADOW_PROJECTORS {
    projectors.select_nth_unstable_by(MAX_SOUL_SHADOW_PROJECTORS, |a, b| {
        a.0.total_cmp(&b.0)
    });
    projectors.truncate(MAX_SOUL_SHADOW_PROJECTORS);
}
// 12件以下のみ sort（コスト無視できる）
projectors.sort_by(|a, b| a.0.total_cmp(&b.0));
```

**Step 2 (オプション)**: Soul が動いていないフレームをスキップ。

```rust
// システム引数に追加
q_souls_changed: Query<(), (With<DamnedSoul>, Changed<Transform>)>,
// ...
if q_souls_changed.is_empty() {
    return;
}
```

ただし Step 2 はカメラ移動（Soulの相対距離変化）でも更新が要るため、カメラ変化も検知が必要。Step 1 だけで十分な効果が見込める。

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/bevy_app/src/systems/visual/soul_shadow_projector.rs:49` | `sort_by` → `select_nth_unstable_by` + truncate + sort |

---

## P2. Proxy Cleanup — ネストループ解消

### 問題

`character_proxy_3d.rs:164-221` に同一パターンが4関数存在（コードで確認済み）:

```rust
// cleanup_soul_proxy_3d_system (164)
// cleanup_soul_mask_proxy_3d_system (178)
// cleanup_soul_shadow_proxy_3d_system (193)
// cleanup_familiar_proxy_3d_system (208)
for removed_entity in removed.read() {
    for (proxy_entity, proxy) in q_proxies.iter() {   // 全プロキシ走査
        if proxy.owner == removed_entity {
            commands.entity(proxy_entity).despawn();
        }
    }
}
```

Soul 削除 1体で全プロキシ (n体) を3回走査。  
削除 k体 × プロキシ数 n × 3種 = O(k × n) 毎削除。

各プロキシコンポーネントは `hw_visual/src/visual3d.rs` で `owner: Entity` を平フィールドで保持（確認済み）:
- `SoulProxy3d` (line 18): `pub owner: Entity, pub billboard: bool`
- `SoulMaskProxy3d` (line 25): `pub owner: Entity`
- `SoulShadowProxy3d` (line 32): `pub owner: Entity`
- `FamiliarProxy3d`（同ファイル内）: `pub owner: Entity`

### 改善

**案A: Bevy 0.18 Relationship による自動 despawn**

`SoulProxy3d` 等は `owner: Entity` を平フィールドで保持している (`hw_visual/src/visual3d.rs:18`)。
カスタム Relationship を owner に張れば、owner despawn 時に proxy が自動 despawn される。
この場合 **cleanup 4関数自体が不要** になり、HashMap キャッシュも register system も要らない。

⚠️ **Bevy 0.18 の auto-despawn 要件**: `ChildOf` は `cleanup_policy = CleanupPolicy::Recursive` を内包しているため owner despawn で子が自動削除されるが、**汎用カスタム Relationship はこの policy を持たない**。案A を実装する場合は Relationship 定義に `cleanup_policy` を明示的に設定する必要がある。実装前に `~/.cargo/registry/src/` の Bevy 0.18 Relationship ソースで `CleanupPolicy` の指定方法を確認すること。

**制約**: proxy は 3D RTT カメラ配下のエンティティ階層に配置される必要がある。
owner (2D Soul) の子にすると RTT カメラのレンダリング対象から外れる可能性がある。
→ **実装前に proxy のエンティティ階層を確認し、owner の子にできるか検証すること。**

上記制約と cleanup_policy 要件を踏まえ、**案B の方が確実性が高い**。

**案B（推奨）: HashMap キャッシュ**

Relationship の階層制約を回避し確実に O(1) で動作する。`Resource` で owner → proxy_entity マップを保持する。

```rust
#[derive(Resource, Default)]
pub struct SoulProxyOwnerCache {
    pub soul_proxy:        HashMap<Entity, Entity>,  // owner → SoulProxy3d entity
    pub soul_mask_proxy:   HashMap<Entity, Entity>,
    pub soul_shadow_proxy: HashMap<Entity, Entity>,
    pub familiar_proxy:    HashMap<Entity, Entity>,
}
```

キャッシュ登録は spawn 時（`Added<SoulProxy3d>` 等）に行う専用システムを追加:

```rust
pub fn register_soul_proxy_3d_system(
    q_new: Query<(Entity, &SoulProxy3d), Added<SoulProxy3d>>,
    mut cache: ResMut<SoulProxyOwnerCache>,
) {
    for (proxy_entity, proxy) in q_new.iter() {
        cache.soul_proxy.insert(proxy.owner, proxy_entity);
    }
}
```

cleanup 側は `cache.soul_proxy.remove(&removed_entity)` で O(1) ルックアップ。

### 変更ファイル

**案A の場合:**

| ファイル | 変更内容 |
|---|---|
| `crates/hw_visual/src/visual3d.rs` | `owner: Entity` → Relationship に変更（4 struct） |
| `crates/bevy_app/src/systems/visual/character_proxy_3d.rs` | cleanup × 4 関数を削除 |
| `crates/bevy_app/src/systems/visual/character_proxy_3d.rs` | spawn 時に Relationship を設定 |
| `crates/bevy_app/src/plugins/visual.rs` | cleanup system の登録を削除 |

**案B の場合:**

| ファイル | 変更内容 |
|---|---|
| `crates/hw_visual/src/visual3d.rs` | `SoulProxyOwnerCache` Resource 定義 |
| `crates/bevy_app/src/systems/visual/character_proxy_3d.rs` | cleanup × 4 関数を O(1) に変更 + register system 追加 |
| `crates/bevy_app/src/plugins/visual.rs` | register system を `GameSystemSet::Visual` に登録 |

---

## P3. Task Area Material — `time` フィールド削除

### 問題

`task_area_visual.rs:38-64`（`bevy_app` 側システム）

```rust
for (visual, material_handle) in q_visuals.iter() {
    if let Some(material) = materials.get_mut(&material_handle.0) {  // 毎フレーム get_mut → 常時 dirty
        material.time = time.elapsed_secs();   // 行 40: 毎フレーム必ず変化

        if let Ok((fam_entity, area)) = q_familiars.get(visual.familiar) {
            material.size = area.size();
            // ...
            material.state = state;  // 行 62
        }
    }
}
```

**問題点が2つある**:

1. `materials.get_mut()` を呼ぶだけで Bevy のアセットシステムがそのマテリアルを「変更済み」と
   マークし、GPU への再アップロードをスケジュールする。現在は毎フレーム・全 Familiar 分だけ発生。
2. `TaskAreaMaterial.time` は `task_area.wgsl:81` で `sin(material.time * 12.0)` に使用されるが、
   `state == 3u`（Editing モード）の境界線描画時のみ。非 Editing フレームでも毎フレーム書き込まれる。

`TaskAreaMaterial` 定義は `hw_visual/src/task_area_visual.rs:8-17`:
- `color: LinearRgba`、`size: Vec2`、`time: f32`、`state: u32` の4フィールド

### 改善

**Step 1 — WGSL 側 (`assets/shaders/task_area.wgsl`)**:

`globals` を追加インポートし `material.time` を `globals.time` へ置換:

```wgsl
// 既存の import を拡張
#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::{view, globals},
}

// struct TaskAreaMaterial から time フィールドを削除
// before: time: f32,  // offset 24
// state: u32,         // offset 28
// after:  state: u32, // offset 24（オフセット詰め直し）

// 行 81:
// before: pulse = 0.8 + 0.2 * sin(material.time * 12.0);
// after:  pulse = 0.8 + 0.2 * sin(globals.time * 12.0);
```

**Step 2 — Rust 側 (`hw_visual/src/task_area_visual.rs`)**:

```rust
pub struct TaskAreaMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub size: Vec2,
    // time フィールドを削除
    #[uniform(0)]
    pub state: u32,
}
```

**Step 3 — システム側 (`bevy_app/src/systems/visual/task_area_visual.rs`)**:

`materials.get_mut()` を変化があった場合のみ呼ぶように再構成:

```rust
// time 引数を削除
for (visual, material_handle) in q_visuals.iter() {
    let Ok((fam_entity, area)) = q_familiars.get(visual.familiar) else {
        continue;
    };
    let new_size = area.size();
    let new_state = /* 既存の state 計算ロジック */ ...;

    // get() で読み取り → 変化がある場合のみ get_mut() で書き込み
    // （get_mut() は呼んだ時点で dirty マークされる可能性があるため）
    let Some(material) = materials.get(&material_handle.0) else {
        continue;
    };
    if material.state == new_state && material.size == new_size {
        continue;
    }
    let Some(material) = materials.get_mut(&material_handle.0) else {
        continue;
    };
    material.size = new_size;
    material.state = new_state;
}
```

`Time` リソース引数も不要になるので削除する。

⚠️ **実装前確認**: Bevy 0.18 の `Assets::get_mut` が**取得時点**で dirty マークするか確認すること（`~/.cargo/registry/src/` の `bevy_asset` ソース）。dirty マークが書き込み後の場合は `get()` + `get_mut()` の2段階は不要で、`get_mut()` 1回で十分。

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/hw_visual/src/task_area_visual.rs` | `time: f32` フィールド削除 |
| `assets/shaders/task_area.wgsl` | `globals` import 追加、`material.time` → `globals.time`、struct offset 詰め直し |
| `crates/bevy_app/src/systems/visual/task_area_visual.rs` | `Time` 引数削除、`material.time = ...` 削除、`materials.get_mut()` を変化時のみ呼ぶよう再構成 |

---

## P4. Producer — Familiar/Yard Vec 毎フレーム再構築

### 問題

**15箇所** で同一パターン（`active_familiars` 8箇所 + `active_yards` 8箇所）:

**`active_familiars` collect (8箇所):**

| ファイル | 行 | 方式 |
|---|---|---|
| `producer/mixer_helpers/collect.rs:27` | `collect_active_familiars()` 関数定義 | 関数定義元 |
| `producer/mixer.rs:50` | `mixer_helpers::collect_active_familiars()` 呼び出し | 共通関数経由 |
| `producer/floor_construction.rs:63` | インライン collect | 直接 |
| `producer/wall_construction.rs:78` | インライン collect | 直接 |
| `producer/provisional_wall.rs:69` | インライン collect | 直接 |
| `producer/blueprint.rs:45` | インライン collect | 直接 |
| `producer/tank_water_request.rs:29` | インライン collect | 直接 |
| `producer/wheelbarrow.rs:45` | インライン collect | 直接 |
| `producer/bucket.rs:101` | インライン collect（当初計画に欠落していた） | 直接 |

**`active_yards` collect (8箇所):**

| ファイル | 行 | 方式 |
|---|---|---|
| `producer/mixer_helpers/collect.rs:37` | `collect_active_yards()` 関数定義 | 関数定義元 |
| `producer/mixer.rs:51` | `mixer_helpers::collect_active_yards()` 呼び出し | 共通関数経由 |
| `producer/wall_construction.rs:83` | インライン collect | 直接 |
| `producer/consolidation.rs:49` | インライン collect | 直接 |
| `producer/task_area.rs:240` | インライン collect | 直接 |
| `producer/bucket.rs:107` | インライン collect | 直接 |
| `producer/provisional_wall.rs:74` | インライン collect | 直接 |
| `producer/tank_water_request.rs:34` | インライン collect | 直接 |
| `producer/blueprint.rs:50` | インライン collect | 直接 |

これらは Logic フェーズの `TransportRequest::Decide` で毎フレーム実行される。
`ActiveCommand` の変化は稀 (ユーザー操作時のみ)。Yard の変化も同様。

**合計: 9 呼び出しサイト（familiar） + 9 呼び出しサイト（yards）、うち 2 つは共通関数定義**

### 改善

`ActiveCommand` / `Yard` の Change Detection ベースで `Resource` にキャッシュ。

```rust
#[derive(Resource, Default)]
pub struct CachedActiveFamiliars(pub Vec<(Entity, AreaBounds)>);

pub fn update_active_familiars_cache(
    q: Query<(Entity, &ActiveCommand, &TaskArea),
             Or<(Added<ActiveCommand>, Changed<ActiveCommand>, Added<TaskArea>, Changed<TaskArea>)>>,
    mut removed: RemovedComponents<ActiveCommand>,
    mut cache: ResMut<CachedActiveFamiliars>,
    q_all: Query<(Entity, &ActiveCommand, &TaskArea)>,
) {
    if q.is_empty() && removed.read().next().is_none() {
        return;  // 変化なし → スキップ
    }
    // 変化があった場合のみ全再構築（頻度は低い）
    cache.0 = q_all
        .iter()
        .filter(|(_, cmd, _)| !matches!(cmd.command, FamiliarCommand::Idle))
        .map(|(e, _, area)| (e, area.bounds()))
        .collect();
}
```

producer 側は `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` を参照するだけになる。

注意: キャッシュ更新システムは producer の Decide フェーズより**前**に登録する必要がある（`TransportRequest::Perceive` 相当の位置）。

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/hw_logistics/src/transport_request/producer/mod.rs` か新規 | `CachedActiveFamiliars` / `CachedActiveYards` Resource 定義 + 更新システム |
| `crates/hw_logistics/src/transport_request/plugin.rs` | キャッシュ更新システムを Perceive フェーズに登録 |
| `producer/mixer_helpers/collect.rs` | `collect_active_familiars` / `collect_active_yards` を削除または非推奨化 |
| `producer/mixer.rs` | `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` 参照に変更 |
| `producer/floor_construction.rs` | インライン collect → `Res<CachedActiveFamiliars>` 参照に変更 |
| `producer/wall_construction.rs` | インライン collect → `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` 参照に変更 |
| `producer/provisional_wall.rs` | インライン collect → `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` 参照に変更 |
| `producer/blueprint.rs` | インライン collect → `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` 参照に変更 |
| `producer/tank_water_request.rs` | インライン collect → `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` 参照に変更 |
| `producer/wheelbarrow.rs` | インライン collect → `Res<CachedActiveFamiliars>` 参照に変更 |
| `producer/task_area.rs` | インライン collect → `Res<CachedActiveYards>` 参照に変更 |
| `producer/consolidation.rs` | インライン collect → `Res<CachedActiveYards>` 参照に変更 |
| `producer/bucket.rs` | インライン collect → `Res<CachedActiveFamiliars>` / `Res<CachedActiveYards>` 参照に変更 |

各 producer システムから `Query<(Entity, &ActiveCommand, &TaskArea)>` / `Query<(Entity, &Yard)>` 引数も削除すること。

---

## P5. Floor/Wall Construction Producer — 全タイル走査の差分化

### 問題

**`floor_construction.rs:70-89`** で毎フレーム全 FloorTile を走査:

```rust
let mut waiting_by_site = HashMap::new();
for tile in q_tiles.iter() {   // 全タイル走査（建設中タイル数に比例）
    match tile.state {
        FloorTileState::WaitingBones => { ... }
        FloorTileState::WaitingMud   => { ... }
        _ => {}
    }
}
```

**`wall_construction.rs:87`** にも同一パターンが存在:

```rust
for tile in q_tiles.iter() {   // 全 WallTile 走査
    match tile.state {
        WallTileState::WaitingWood => { ... }
        _ => {}
    }
}
```

両者とも `floor_construction_auto_haul_system` / `wall_construction_auto_haul_system` で毎フレーム実行される。建設中タイル数が多い場合の負荷に比例して問題が大きくなる。

### 改善

`FloorTileBlueprint` / `WallTileBlueprint` の状態変化時のみサイト別集計を更新する `Resource` を導入。

```rust
#[derive(Resource, Default)]
pub struct FloorTileWaitingCache(pub HashMap<Entity, (u32, u32)>);
// key: parent_site, value: (waiting_bones, waiting_mud)

pub fn update_floor_tile_waiting_cache(
    q_changed: Query<&FloorTileBlueprint, Changed<FloorTileBlueprint>>,
    q_all: Query<&FloorTileBlueprint>,
    mut cache: ResMut<FloorTileWaitingCache>,
) {
    if q_changed.is_empty() { return; }
    // 変化があったサイトのみ再集計（または全再構築でも頻度が低ければ問題ない）
    cache.0.clear();
    for tile in q_all.iter() {
        // ... 集計
    }
}
```

producer は `Res<FloorTileWaitingCache>` / `Res<WallTileWaitingCache>` を参照するだけ。

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/hw_logistics/src/transport_request/producer/floor_construction.rs` | キャッシュ参照に変更（`FloorTileWaitingCache`） |
| `crates/hw_logistics/src/transport_request/producer/wall_construction.rs` | キャッシュ参照に変更（`WallTileWaitingCache`） |
| 新規ファイル or `plugin.rs` | `FloorTileWaitingCache` / `WallTileWaitingCache` Resource 定義 + 更新システム |
| `crates/hw_logistics/src/transport_request/plugin.rs` | キャッシュ更新システムを Perceive フェーズに登録 |

---

## P6. Assignment Loop — `to_vec()` 削除

### 問題

`assignment_loop.rs:117-119`

```rust
ranked.select_nth_unstable_by(top_k, |a, b| { b.1.partial_cmp(&a.1)... });
let mut top_ranked = ranked[..top_k].to_vec();    // ヒープ確保 1
top_ranked.sort_by(...);
let fallback_ranked = ranked[top_k..].to_vec();   // ヒープ確保 2
(
    top_ranked.into_iter().map(|(c, _)| c).collect(),  // ヒープ確保 3
    fallback_ranked,
)
```

`select_nth_unstable_by` 後のスライスをそのまま使えば中間 Vec を削減できる。

### 改善

```rust
ranked.select_nth_unstable_by(top_k, |a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
ranked[..top_k].sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
let top: Vec<DelegationCandidate> = ranked[..top_k].iter().map(|(c, _)| *c).collect();
let fallback: Vec<(DelegationCandidate, f32)> = ranked[top_k..].to_vec();  // fallback は型が必要なので維持
(top, fallback)
```

ヒープ確保が 3回 → 2回になる。小さいが委譲ティック（Familiar 数 × フレーム）で積み重なる。

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/delegation/assignment_loop.rs:117-128` | `to_vec()` 削除、in-place ソート |

---

## P7. DreamBubble UI Material — `time` フィールド削除

### 問題

`hw_visual/src/dream/dream_bubble_material.rs:29-40`:

```rust
pub struct DreamBubbleUiMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(0)]
    pub time: f32,          // ← 毎フレーム CPU 書き込み
    #[uniform(0)]
    pub alpha: f32,
    #[uniform(0)]
    pub mass: f32,
    #[uniform(0)]
    pub velocity_dir: Vec2,
}
```

`time` を書き込む箇所が **3ファイル**（コードで確認済み）:

| ファイル | 行 | 処理 |
|---|---|---|
| `update_standard.rs:320` | `mat.time = elapsed;` | 毎フレーム更新（`materials.get_mut()` も伴う） |
| `update.rs:196` | `mat.time = elapsed;` | merging パーティクル更新 |
| `update_trail.rs:57` | `time: elapsed` | trail spawn 時の初期値 |

**参考**: world-space 版（`DreamBubbleMaterial` / `dream_bubble.wgsl`）は
既に `globals.time` を使用済み（`dream_bubble.wgsl:7` で import 確認済み）。
UI 版でも同じ対応が可能。

`dream_bubble_ui.wgsl` では `material.time` が多数の箇所で参照されている
（`bubble_at` 関数の引数として渡されるため1箇所にまとまっていない）。

### 改善

**Bevy 0.18 の UiMaterial における `globals` バインディング**:

⚠️ **実装前確認必須**: `UiMaterial` は `Material2d` とは別パイプライン（`bevy_ui_render`）を使用する。`@group(0)` の binding layout が `Material2d` と同一とは限らない。**実装前に `~/.cargo/registry/src/` の `bevy_ui_render` ソース（`ui_material.wgsl` 等）で `@group(0) @binding(N)` の実際の割り当てを確認すること**。

想定される layout（要確認）:
```wgsl
// @group(0) @binding(0) var<uniform> view: View;
// @group(0) @binding(1) var<uniform> globals: Globals;
```

確認後、以下を実施:
```wgsl
// dream_bubble_ui.wgsl の import 先頭に追加:
#import bevy_render::{
    view::View,
    globals::Globals,
}

@group(0) @binding(1) var<uniform> globals: Globals;  // binding 番号は確認した値に合わせる

// 全 material.time 参照を globals.time に置換
// 例: let t = material.time * 0.8;  →  let t = globals.time * 0.8;
```

**Rust 側 (`dream_bubble_material.rs`)**:

```rust
pub struct DreamBubbleUiMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    // time フィールドを削除
    #[uniform(0)]
    pub alpha: f32,
    #[uniform(0)]
    pub mass: f32,
    #[uniform(0)]
    pub velocity_dir: Vec2,
}
```

### 変更ファイル

| ファイル | 変更内容 |
|---|---|
| `crates/hw_visual/src/dream/dream_bubble_material.rs` | `DreamBubbleUiMaterial` から `time: f32` 削除 |
| `assets/shaders/dream_bubble_ui.wgsl` | `bevy_render::globals::Globals` import + `@group(0) @binding(1)` 宣言、全 `material.time` → `globals.time` |
| `crates/hw_visual/src/dream/ui_particle/update/update_standard.rs:320` | `mat.time = elapsed;` 行を削除 |
| `crates/hw_visual/src/dream/ui_particle/update.rs:196` | `mat.time = elapsed;` 行を削除 |
| `crates/hw_visual/src/dream/ui_particle/update/update_trail.rs:57` | spawn 時の `time: elapsed` フィールドを削除 |

---

## 実施順序

依存関係なし。独立して実施可能。推奨順:

```
P1 → P3 → P7 → P6 → P2 → P5 → P4
↑小工数・即効        ↑要検証  ↑大工数
```

| ステップ | 理由 |
|---|---|
| P1 先行 | 1ファイル変更・即効性高 |
| P3・P7 セット | WGSL `globals.time` パターンが同一・小工数 |
| P6 | 1ファイル・確実な改善 |
| P2 | Relationship 代替案の検証結果で方針が変わるため、先に検証してから実施 |
| P5 | キャッシュ設計が P4 と類似・P4 の前に小規模で練習 |
| P4 最後 | 14ファイル変更・影響範囲が最大 |

---

## 完了条件

各項目ともに:

1. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` — エラーなし
2. `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo clippy --workspace 2>&1 | grep "^warning:" | grep -v generated` — 0件
3. ゲーム起動、Soul 多数スポーン・建設・タスク割当を実行して挙動変化なし確認
4. P3/P7: コンソールに `wgpu error` なし
