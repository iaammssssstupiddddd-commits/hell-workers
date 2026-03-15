# Phase 2 実装計画 レビュー

> 対象：`phase2-hybrid-rtt-plan-2026-03-15.md`
> レビュー観点：設計上の前提認識・技術的実装の正確性

---

## 0. 前提の整理（最重要）

レビューの出発点として、Phase 2 におけるプリミティブの位置づけを明確にする。

```
Phase 2（現在）
  └─ Cuboid / Plane3d = 技術検証用プレースホルダー
     目的：Zバッファ動作確認・矢視機能・RtT合成パイプラインの検証
     アートスタイル確立は明示的に非対象

Phase 3（将来）
  └─ AI生成 GLB = 実際のゲームアセット
     目的：地形3D化・Raycasting化・プリミティブをGLBに差し替え
```

ロードマップの MS-2A 非対象欄にも「3Dモデルのアートスタイル確立（仮 Cuboid / Plane3d のまま）」と明記されている。

**しかし現在の計画書は、いくつかの箇所でプリミティブを本実装として扱っており、Phase 3 での GLB 差し替えコストが増大する設計になっている。** 以下のレビューはこの前提を基準に記述する。

---

## 1. 重大度：高（実装前に修正必須）

### 1-1. 壁接続ロジックの削除範囲と Phase 3 への影響確認

**該当箇所**：MS-2A M2A-3

計画書の記述：
> 「完成Building向けスプライト更新ブロックを削除」

実際の `wall_connection.rs` の構造を確認すると、「層A（検出）／層B（スプライト切替）」のクリーンな分離はない。`update_wall_sprite()` が隣接検出（`is_wall()` の4方向呼び出し）とスプライト選択を同関数内で担っている。

M2A-3 が削除するのは **完成Building パス（`q_children.get(entity)` ブロック）のみ** であり、Blueprint パスと `is_wall()` / `add_neighbors_to_update()` / `update_wall_sprite()` はすべて Blueprint パスを通じて残存する。

```
wall_connections_system の処理
├─ 完成Building パス (q_children.get → VisualLayerKind::Struct)  ← M2A-3 で削除
└─ Blueprint パス (q_blueprint_sprites.get_mut)                  ← 残存
      共通ヘルパー: is_wall(), add_neighbors_to_update(), update_wall_sprite()
```

Phase 3 で GLB 壁を「直線/コーナー/T字/十字」に分ける場合も、`is_wall()` は Blueprint パス経由で生き残るため**再実装は不要**。

**修正方針**：削除は完成Building パスのみと確認した。計画書のM2A-3に「削除対象は`q_children.get(entity)`ブロック（完成Building パス）のみ；`is_wall()`等のヘルパーは Blueprint パスを通じて保持される」と注記を追加する。

---

### 1-2. 壁の 2D Sprite エンティティが残存したまま 3D Cuboid が追加される

**該当箇所**：MS-2A M2A-2

現状の `spawn_completed_building` はすべての `BuildingType` に対して `VisualLayerKind::Struct` を持つ子エンティティ（2D Sprite）を生成する。この子エンティティは `RenderLayers` が未指定のためデフォルトの `layer(0)`（Camera2d 側）に描画される。

M2A-2 で `Building3dVisual`（3D Cuboid、`layer(1)`）を追加すると Camera3d → RtT → Camera2d で合成されるが、元の 2D Sprite エンティティも同じ Camera2d に残り続ける。結果として壁が以下の 2 回描画される。

```
Camera2d（LAYER_2D）
  ├─ 元の 2D Sprite（VisualLayerKind::Struct） ← 残存
  └─ RtT 合成スプライト（3D Cuboid を含む）   ← 新規追加
```

**修正方針**：`spawn.rs` の `BuildingType::Wall` 分岐で `VisualLayerKind::Struct` 子エンティティの Spawn をスキップするか、`Visibility::Hidden` を設定する。

```rust
// spawn.rs（修正案）
match bp.kind {
    BuildingType::Wall => {
        // 3D Cuboid で代替するため 2D Sprite は生成しない
        // または Visibility::Hidden を付与する
    }
    _ => {
        // 既存の子エンティティ Spawn
    }
}
```

---

### 1-3. Pre-2 で合成スプライトを削除後、Phase 2 全体を通じて RtT を画面に表示する計画がない【P0】

**該当箇所**：MS-Pre2（前提タスク）・MS-2A〜2D 全体

**ファイル誤記の訂正**：`rtt_setup.rs` は `RttTextures` リソースと `Camera3dRtt` マーカーのみを定義しており、合成スプライトは含まない。合成スプライト（Z=20.0 のローカル座標、Camera2d の子エンティティとして追従）は `rtt_test_scene.rs` に定義されている。

**真の問題**：`rtt_test_scene.rs` は冒頭コメントに「M4 検証完了後、Phase 2 開始前に削除すること」と明記されている。Pre-2 で削除されると、**Phase 2 の実装期間を通じて RtT テクスチャを Camera2d に表示する仕組みが存在しなくなる**。

```
現状（Phase 1 完了時）
  rtt_test_scene.rs
    └─ spawn_rtt_composite_sprite: Sprite(rtt.texture_3d) を Camera2d 子エンティティとして Z=20.0 に spawn

Pre-2 後（Phase 2 期間全体）
  ← rtt_test_scene.rs が削除される
  ← 合成スプライトを spawn する仕組みが存在しない
  ← Camera3d が 3D オブジェクトを RtT に描画しても画面には何も表示されない
```

Phase 2 で壁 Cuboid を 3D 化しても、RtT 合成スプライトがなければ Camera2d 上には映らず、検証もできない。

**修正方針**：Pre-2 のタスクに「`rtt_composite.rs`（仮）などの恒久的な合成スプライト spawn システムを作成し、`rtt_test_scene.rs` を削除する前に移行する」ステップを追加する。また MS-2C の Z 値整合も同時に明記する。

```
Camera2d（LAYER_2D）で描画されるもの
  ├─ RtT 合成スプライト（恒久化後・Z 値要確定）← rtt_test_scene.rs 削除前に移行必須
  ├─ 地形タイル（2D Sprite）
  ├─ ResourceItem スプライト（Z_ITEM = 0.10）
  └─ UI など
```

合成スプライトの Z 値が `Z_ITEM（0.10）` より小さければアイテムが壁の上に表示され、大きければアイテムが壁の下に隠れる。現状の仮実装 Z=20.0 は最前面だが、2D UI との重なりが発生する可能性があり、MS-2C で要確認。

---

## 2. 重大度：中（着手前に確認が必要）

### 2-1. プリミティブを本実装として扱っている箇所が複数ある

**該当箇所**：計画書全体

以下の箇所でプリミティブが仮であるという認識が欠落している。

**MS-2D の建築物テーブル**：

| BuildingType | 使用3Dプリミティブ | 注記 |
|:---|:---|:---|
| Tank | Cuboid | 「仮モデル」あり |
| MudMixer | Cuboid | 「仮モデル」あり |
| Floor | Plane3d | 「仮」注記なし |
| Bridge | Plane3d | 「仮」注記なし |
| SandPile | Plane3d | 「仮」注記なし |

Tank・MudMixer には「仮モデル」注記があるが、Floor・Bridge・SandPile・BonePile には注記がなく、Plane3d がそのまま Phase 3 でも使われる前提に見える。Plane3d は厚みゼロの水平面であり、Phase 3 で厚みのある床メッシュに変更した場合、Zファイティングの再調整が必要になる。

**MS-2A / MS-2D の完了条件**：

> 「トップダウン視点で壁の見た目が正しく表示される」

「正しい見た目」が Cuboid のグレーブロックを指すのか、world_lore のアートスタイルを指すのかが曖昧。完了条件の表現が Phase 3 まで使いまわされると誤った判断につながる。

**修正方針**：計画書の冒頭「4. 実装方針」セクションに以下を追記する。

> Phase 2 のプリミティブは Zバッファ・矢視・RtT パイプラインを技術的に検証するための仮実装である。`Building3dHandles` の `Handle<Mesh>` は Phase 3 で GLB に差し替えることを前提に設計する。壁接続の隣接検出ロジック（`wall_connection.rs` の層 A 相当）は Phase 3 で再利用できる形で保持するか、削除する場合は Phase 3 での再実装コストを見積もりに含める。

さらに全 BuildingType に「仮（Phase 3 で GLB に差し替え対象）」の注記を付け、各 MS の完了条件に「プリミティブとして」という限定を明示する。

---

### 2-2. 仮設壁（`is_provisional: true`）の 3D 表現方針【決定済み】

**該当箇所**：MS-2A

**方針 B を採用**：仮設壁にも 3D Cuboid を spawn するが、完成壁とは異なる色のマテリアルで区別する。

```
完成壁：通常マテリアル（グレー等）
仮設壁：警告色マテリアル（橙 / 半透明 等）← 2D の srgba(1.0, 0.75, 0.4, 0.85) に準拠
```

**追加実装が必要な要素**：
- `spawn_completed_building`（またはその上流）で `building.is_provisional` を確認し、マテリアルを分岐する
- 仮設→本設（`CoatWall` 完了）の遷移時に `Building3dVisual` エンティティのマテリアルを差し替えるシステム（`Changed<Building>` を監視）が別途必要

これらを MS-2A の変更ファイルリストと完了条件に追記すること。

---

### 2-3. MS-Elev で `sync_camera3d_system` を単純スキップするとパン追従が失われる

**該当箇所**：MS-Elev M-Elev-2

計画書の記述：
> 「`ElevationViewState != TopDown` のとき `sync_camera3d_system` をスキップする条件分岐を追加すること」

矢視中もプレイヤーはパン操作でシーンを動かすことができる必要がある。単純スキップでは Camera2d のパン位置が Camera3d に反映されなくなり、矢視中にパンが無効化される。

**修正方針**：矢視中は「平行移動のみ同期し、回転・角度は同期しない」という分岐が必要。

```rust
// camera_sync.rs（修正案）
fn sync_camera3d_system(...) {
    match elevation_state {
        ElevationViewState::TopDown => {
            // 現行の全同期（既存処理）
        }
        _ => {
            // 矢視中：XZ 平面の平行移動のみ同期、向きは上書きしない
            cam3d.translation.x = cam2d.translation.x;
            cam3d.scale = cam2d.scale;
            // cam3d.translation.z は矢視プリセットが管理するため同期しない
        }
    }
}
```

---

### 2-4. キャラクタープロキシのメッシュサイズが数値直書きになっている

**該当箇所**：MS-2B M2B-2

計画書の記述：
> 「Soul プロキシ: `Cuboid::new(TILE_SIZE * 0.6, TILE_SIZE * 0.8, TILE_SIZE * 0.6)` 相当の仮メッシュ」

このサイズは Zバッファ検証のための仮値だが、Spawn コードに直書きされると Phase 3 で GLB に差し替えるときに残骸として残る。また `0.6` `0.8` といった係数の根拠が記載されておらず、後から変更した場合の影響範囲が分かりにくい。

**修正方針**：`Building3dHandles` 経由で参照する設計にし、直書きの数値にはコメントで「仮・Phase 3 で GLB 差し替え対象」と明記する。

```rust
// 仮サイズ（Phase 3 で GLB に差し替え対象）
// Soul のスプライトサイズ（約 0.6 タイル）に合わせた Zバッファ検証用プレースホルダー
soul_mesh: meshes.add(Cuboid::new(TILE_SIZE * 0.6, TILE_SIZE * 0.8, TILE_SIZE * 0.6)),
```

---

## 3. 重大度：低（確認推奨）

### 3-1. `spawn_completed_building` 呼び出し元が変更ファイルリストに含まれていない

**該当箇所**：MS-2A 変更ファイルリスト

計画書では「`spawn_completed_building` 関数に `Building3dHandles` を引数追加」とあるが、実際の呼び出し元は `building_completion/mod.rs`（`building_completion_system` 関数内 line 35）であり、そのシステムシグネチャにも `Res<Building3dHandles>` の追加が必要になる。変更ファイルリストに `building_completion/mod.rs` が含まれておらず、実装中に発覚すると手が止まる。

---

### 3-2. 矢視中の 2D Sprite エンティティ（アイテム・Stockpile 等）の扱いが未定義

**該当箇所**：MS-Elev M-Elev-3

計画書では地形タイルのみ `Visibility::Hidden` にする記述があるが、矢視中に ResourceItem や Stockpile などの 2D Sprite が画面中央付近に浮いて見える問題が発生する可能性がある。矢視の用途が「建物を横から確認する」ことであれば、2D オブジェクト全体を非表示にするかどうかの方針を決めておく必要がある。

---

## 4. 修正優先順位まとめ

| 優先度 | 箇所 | 修正内容 | 理由 |
|:---:|:---|:---|:---|
| P0 | 計画書冒頭 | 「プリミティブは仮実装・Phase 3 で GLB 差し替え前提」の原則を明記する | 以降の混同を防ぐ根本対策 |
| P0 | MS-Pre2 | `rtt_test_scene.rs` 削除前に恒久的な合成スプライト spawn システムへ移行する | 削除後 Phase 2 全体で RtT が画面に映らなくなる |
| P0 | MS-2A M2A-2 | `BuildingType::Wall` の 2D Sprite を非表示/スキップする処理を `spawn.rs` に追加する | MS-2A 完了直後から二重表示が発生する |
| P0 | MS-2C | 恒久化した合成スプライトの Z 値と `Z_ITEM`（0.10）との整合確認・検証シナリオを追記する | MS-2C で発見されるが計画がないと対処が遅れる |
| P1 | MS-2A M2A-3 | M2A-3 の削除対象が完成Building パスのみ（`q_children.get` ブロック）であることを明記する；`is_wall()` 等のヘルパーは Blueprint パスを通じて保持される | 計画書の記述が「全削除」と誤解されやすく、Phase 3 への影響範囲が不明確 |
| P1 | MS-2D 全体 | 全 BuildingType に「仮（Phase 3 で GLB 差し替え対象）」注記を追加する | プリミティブを本実装と混同した最適化を防ぐ |
| P1 | 各 MS 完了条件 | 「プリミティブとして」という限定を各完了条件に明示する | Phase 3 へ引き継ぐ際の判断基準を明確にする |
| P1 | MS-2A | 仮設壁の 3D 表現方針（生成するか・マテリアルをどう区別するか）を明記する | 実装中に設計不足で手が止まる |
| P1 | MS-Elev M-Elev-2 | `sync_camera3d_system` の矢視中挙動を「全スキップ」から「平行移動のみ同期」に変更する | 矢視中にパン操作が無効になる |
| P2 | MS-2B M2B-2 | キャラクタープロキシのメッシュサイズに「仮・Phase 3 で GLB 差し替え対象」コメントを追加する | Phase 3 での残骸を防ぐ |
| P2 | MS-2A 変更ファイルリスト | `spawn_completed_building` 呼び出し元（`building_completion/mod.rs`）を変更ファイルリストに追加する | 実装中の変更漏れを防ぐ |
| P2 | MS-Elev | 矢視中の 2D Sprite エンティティ（アイテム・Stockpile 等）の表示方針を決定する | 見た目の問題として後から発覚する |
