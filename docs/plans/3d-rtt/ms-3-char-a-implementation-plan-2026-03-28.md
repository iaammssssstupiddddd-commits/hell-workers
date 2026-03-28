# MS-3-Char-A 実装計画（2026-03-28）

## 問題

`MS-3-1` は完了し、Soul は `soul.glb` + `CharacterMaterial` + Soul mask prepass で通常表示できている。次の `MS-3-Char-A` は `AnimationGraph + SoulAnimState` 実装だが、現行アセット仕様と現行コードの間に以下のギャップがある。

- 既存計画は `Idle / Walk` 前提で書かれているが、現行 GLB は `Carry / Exhausted / Fear / Idle / Walk / WalkLeft / WalkRight / Work` を含む
- face atlas は `通常 / 恐怖 / 疲弊 / 集中 / 喜び / 睡眠` まで揃っている
- 現行コードは Soul ごとの face material を持っておらず、全 Soul が共有 `Handle<CharacterMaterial>` を参照している
- `AnimationPlayer` / `AnimationGraphHandle` / `AnimationTransitions` を Soul GLB へ結び付ける経路が未実装
- 旧 2D sprite 系の表情選択ロジックは残っているが、3D 側でそのまま再利用できる形には整理されていない

このため、`MS-3-Char-A` は「Idle/Walk を動かす」だけでなく、後続の `MS-3-Char-B` で破綻しないデータ構造へ先に寄せる必要がある。

## 現行アセット仕様

### Soul GLB

出典: `assets/models/characters/animation_list.md`

| 種別 | クリップ |
| --- | --- |
| 体アニメ | `Idle`, `Walk`, `Work`, `Carry`, `Fear`, `Exhausted` |
| 追加歩行 | `WalkLeft`, `WalkRight` |

方針:

- `MS-3-Char-A` では `Idle` / `Walk` を必須接続対象にする
- `Work` / `Carry` / `Fear` / `Exhausted` は同じ registry に読み込んでおき、`MS-3-Char-B` で本接続する
- `WalkLeft` / `WalkRight` は現行カメラでは優先度が低いため未使用で保持のみ

### face atlas

出典: `assets/textures/character/soul_face_atlas_layout.md`

| 状態 | atlas セル | 用途 |
| --- | --- | --- |
| Normal | `(0,0)` | 基本顔 |
| Fear | `(1,0)` | 恐怖・ネガティブ |
| Exhausted | `(2,0)` | 疲弊 |
| Focused | `(0,1)` | 作業中 |
| Happy | `(1,1)` | ポジティブ会話 |
| Sleep | `(2,1)` | 睡眠・休憩 |

方針:

- `MS-3-Char-A` で face atlas 切り替え経路自体は入れる
- ただし body clip と 1:1 対応しないため、face 状態は body 状態から分離して管理する

## 設計方針

### 1. `SoulAnimState` は body / face を内包する

`SoulAnimState` を単一 enum にせず、少なくとも概念上は次の 2 系統へ分ける。

- `SoulBodyAnimState`: `Idle / Walk / Work / Carry / Fear / Exhausted`
- `SoulFaceState`: `Normal / Focused / Fear / Exhausted / Happy / Sleep`

実装形は 2 enum + 1 component struct でもよいし、component を分割してもよい。重要なのは、`Happy` と `Sleep` を body clip に無理に押し込まないこと。

### 2. face material は per-instance 化する

現行 `CharacterHandles.soul_face_material` は共有 handle なので、`face_uv_offset` を Soul ごとに変えられない。

`MS-3-Char-A` では次を行う。

- `CharacterHandles` は「face atlas source / body 共通 material / mask material」の template 的責務に寄せる
- `SceneInstanceReady` 時に Soul ごとの face material handle を生成する
- face mesh か Soul root に「更新対象 handle」を保持する component を付ける
- `sync_soul_face_expression_system` がその handle の material uniform を更新する

### 3. animation clip は `Handle<Gltf>` から名前解決する

`animation_list.md` の表をコード定数に直書きせず、Bevy 0.18 の `Gltf.named_animations` を使って clip handle を解決する。

必要な追加:

- `GameAssets` に `Handle<Gltf>` を追加
- `SoulAnimationLibrary` resource を追加
- GLTF 読み込み完了後、`Idle / Walk / Work / Carry / Fear / Exhausted / WalkLeft / WalkRight` を名前で解決して保持

これで GLB 再 export 時の clip index 変動に耐える。

### 4. `MS-3-Char-A` は「基盤 + Idle/Walk + face切替」までに絞る

`MS-3-Char-A` のスコープ:

- AnimationGraph / AnimationTransitions 導入
- Soul ごとの animation player binding
- `Idle` / `Walk` の body clip 切り替え
- face atlas の状態切り替え経路

`MS-3-Char-B` へ送るもの:

- `Work` / `Carry` / `Fear` / `Exhausted` body clip 本接続
- 作業方向・タスク種別の細かい分岐
- face 切り替え規則の最終調整

## 実装ステップ

### Step 1. アセット解決基盤

- `GameAssets` に `soul_gltf: Handle<Gltf>` を追加する
- startup で `SoulAnimationLibrary` resource を初期化する
- `Assets<Gltf>` から `named_animations` を読んで clip handle を解決する system を追加する
- 解決対象は `Idle`, `Walk`, `Work`, `Carry`, `Fear`, `Exhausted`, `WalkLeft`, `WalkRight`

### Step 2. Soul animation binding

- Soul root または GLB 内 `AnimationPlayer` entity を特定する marker / binding component を追加する
- `Added<AnimationPlayer>` を契機に `AnimationGraphHandle` と `AnimationTransitions` を挿入する
- Soul owner と player entity の関連を保持する

### Step 3. `SoulAnimState` 導入

- `SoulAnimState` component を新規追加する
- `sync_soul_anim_state_system` を実装し、以下の入力から body / face 状態を決める
  - `AssignedTask`
  - `AnimationState.is_moving`
  - `IdleState.behavior`
  - `DamnedSoul.fatigue`
  - `StressBreakdown`
  - `ConversationExpression`
- 初期接続は以下で十分:
  - body: `Walk` if moving, else `Idle`
  - face: `Sleep / Happy / Fear / Exhausted / Focused / Normal`

### Step 4. face material per-instance 化

- `apply_soul_gltf_render_layers_on_ready` で face mesh に共有 handle を直接入れず、Soul ごとに `Assets<CharacterMaterial>::add(...)` した handle を設定する
- その handle を更新できる component を face mesh に付与する
- `sync_soul_face_expression_system` で `uv_offset` を atlas layout に従って更新する

### Step 5. body animation 切り替え

- `AnimationGraph` を最小構成で作る
- node は最低 `Idle`, `Walk` を持つ
- `AnimationTransitions::play(..., Duration::from_millis(...))` 経由で切り替える
- `WalkLeft` / `WalkRight` は registry には保持するが未使用のままにする

### Step 6. 旧 2D ロジックからの移植整理

- `movement/animation.rs` の sprite 向け表情選択ロジックを参照し、face 状態判定だけを 3D 側へ移植する
- 2D sprite が存在しない前提で、3D 側と重複する責務を整理する

## 変更ファイル候補

- `crates/bevy_app/src/assets.rs`
- `crates/bevy_app/src/plugins/startup/asset_catalog.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`
- `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/bevy_app/src/entities/damned_soul/movement/animation.rs`
- `crates/hw_visual/src/material/character_material.rs`
- `crates/hw_visual/src/visual3d.rs`
- `crates/hw_visual/src/lib.rs`
- `assets/models/characters/animation_list.md`（参照のみ）
- `assets/textures/character/soul_face_atlas_layout.md`（参照のみ）

必要に応じて新規:

- `crates/hw_visual/src/anim/soul_anim.rs`
- `crates/bevy_app/src/systems/visual/soul_animation.rs`

## 検証

### コード

- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

### 目視

- Soul が待機時に `Idle` を再生する
- 移動時に `Walk` へ切り替わる
- face atlas が `Normal / Sleep / Fear / Exhausted / Focused / Happy` で切り替わる
- Soul mask prepass と body 不透明化に退行がない
- Familiar 表示に退行がない

## リスク

- `AnimationPlayer` が GLB 内の想定しない child に付く可能性がある
- face material を per-instance 化しない限り、全 Soul の表情が同時に変わる
- `WalkLeft` / `WalkRight` を早く使い始めると、現在の斜めカメラ前提と干渉して実装が肥大化する

## 結論

現行アセットは `MS-3-Char-A` 着手に十分で、むしろ `MS-3-Char-B` で使う clip / face atlas まで先に揃っている。したがって次は `Idle / Walk` だけを場当たり的に繋ぐのではなく、

1. `Handle<Gltf>` ベースの clip registry
2. `SoulAnimState` の body / face 分離
3. face material の per-instance 化

を先に入れ、その上で `Idle / Walk` と face atlas 切り替えを成立させるのが最短経路。
