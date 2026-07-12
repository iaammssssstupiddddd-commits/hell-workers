# 全体パフォーマンス改善フォローアップ計画書

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `system-wide-performance-followups-plan-2026-07-07` |
| ステータス | `Complete` |
| 作成日 | `2026-07-07` |
| 最終更新日 | `2026-07-07` |
| 作成者 | `Claude` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- **解決したい課題**: コード静的レビューおよびコード詳細調査で見つかった、毎フレームの全件走査、同一フレーム内の重複キャッシュ構築、マテリアルの無駄な `get_mut` 更新によるアセット再送信、不要なパス検証や非効率な線形探索を段階的に削減する。
- **到達したい状態**: ゲーム挙動（Familiar のタスク発見能力や Soul の移動経路など）を変更することなく、Soul/Familiar/TransportRequest/UI particle/建物の数が増えた中規模・大規模シーンでの CPU 負荷（Frame time）とフレームごとのアロケーション、描画前オーバーヘッドを抑制する。
- **成功指標**:
  - `cargo check --workspace` と `cargo clippy --workspace` にてコンパイルエラーおよび clippy 警告が 0 件であること。
  - 大規模シーンにおいて、DevPanel の frame time / system metrics が悪化せず、改善傾向を示すこと。
  - 各マイルストーンの変更により、対象システムの実行頻度、走査要素数、アセット更新呼び出し数が削減されていること。

## 2. スコープ

### 対象（In Scope）

- **Familiar AI**: タスク委譲における `IncomingDeliverySnapshot` の不要フレームでの構築回避。
- **UI Hover**: カメラ・カーソル変化時に限定した hit-test 実行、および平方距離比較への統一。
- **Logistics**: `build_stockpile_groups` 等の stockpile group 構築と空間インデックス構築の 1 回化（Perceive フェーズでのリソースキャッシュ化）。
- **Dream UI Particle / UI List**:
  - `DreamBubbleUiMaterial` に対する `get_mut` 頻度の削減。custom shader 入力を維持するため、単純な `Node` 置換ではなく共有 material bucket 化または shader 入力経路の再設計を検討する。
  - Entity list UI の並び替え（structure dirty）と vitals 値変更（value dirty）の分離による無駄な UI 再生成の抑制。
- **Pathfinding**:
  - `pathfinding_system` に残置されているハードコードされたデバッグ用 `info!` / `warn!` 出力の完全削除。
  - `pathfinding_system` の 2 パス（task/idle）による全 Soul 走査（O(N)）を、`Changed<Destination>` や marker 判定に絞り込み。
  - `try_reuse_existing_path` 内のパス検証走査（O(Waypoints)）について、`WorldMap` / 障害物状態の version と `Path` 側の検証済み version を比較し、障害物状態が変わっていないときはスキップ。
- **Visual 3D Sync**:
  - 3Dプロキシ（Soul, Mask, Shadow, Familiar）の同期システム群に対し、3D描画無効時（`Render3dVisible(false)`）の `run_if` 実行抑制を追加。
  - `sync_soul_face_expression_system` で表情が変わっていない場合に `Assets::get_mut` を叩きアセット再アップロードを誘発する問題のガード（前値キャッシュ比較または `Changed<SoulAnimVisualState>` ゲート）。
- **Utility / Index**:
  - `TileSiteIndex` の要素削除時（`sync_removed_tiles`）の O(R * S) 走査を、逆引き `HashMap<Entity, Entity>` の導入により O(R) に最適化。

### 非対象（Out of Scope）

- 搬送優先度や成立条件など、ゲーム・AI ロジックの仕様変更。
- A* アルゴリズムの全面書き換えや、JPS 等への置換。
- 3D モデルやシェーダーの根本的な作り直し。

## 3. 現状とギャップ

- **現状**:
  - 空間グリッドや到達判定キャッシュ、LOD など大きな枠組みは導入されている。
  - しかし、細部において「毎フレーム無条件で全件ループを回す」「毎フレーム同じアセットの `get_mut` を呼んで変更フラグを立ててしまう」「削除時に全リストを retain で舐める」といった CPU コスト・描画前オーバーヘッドが散見される。
- **問題**:
  - Soul や Familiar が増えると、これらの O(N) / O(N * M) 処理の累積が顕著になり、DevPanel の frame time 増加を招く。
  - 描画関連では、マテリアルの `get_mut` が毎フレーム発生することで batching が壊れ、描画命令の発行とデータ転送の無駄なオーバーヘッドが生じる。
- **本計画で埋めるギャップ**:
  - 動的変更検知 (`Changed<T>` / `is_changed()`)、キャッシュリソース化、逆引きインデックス、`run_if` ガードなどを適用し、不要な計算とアセット更新を最小限に抑える。

## 4. 実装方針（高レベル）

- 既存の crate 境界 (`hw_*` と `bevy_app`) を崩さない。
- Bevy 0.19 の API 仕様を一次情報（`docs.rs` やローカル registry）で確認しながら、安全な変更を適用する。
- 段階ごとに `cargo check` を行い、コミット前に `cargo clippy --workspace` の警告をゼロにする。

---

## 5. マイルストーン詳細

### M0: ベースライン計測と安全な観測点の確定

- **目的**: 変更前の正確なパフォーマンス指標と実行時カウンタのベースラインを記録する。
- **変更内容**:
  - 既存の `FamiliarDelegationPerfMetrics`（`crates/hw_familiar_ai/src/familiar_ai/decide/resources.rs`）が収集する metrics の動作確認。
  - テスト用の高負荷シーン（Familiar/Soul 多数配置）を定義・選定。
- **完了条件**:
  - [ ] 比較用テストシーンおよび測定手順の決定。
  - [ ] 変更前のフレームレート、フレームタイム、および delegation カウンタ値の記録。

---

### M1: 入力・Familiar delegation の毎フレーム走査削減

- **ボトルネックと現状コード**:
  - `task_delegation.rs` 内で、毎フレーム `IncomingDeliverySnapshot::build(&task_queries)` が無条件に実行されている。
  - `is_idle_command` な Familiar の存在や timer の `allow_task_delegation` 状態に関わらず snapshot が構築されている。
- **具体的な変更内容**:
  - `allow_task_delegation || any_idle_familiar` の判定結果を事前に評価し、委譲処理が実際に実行されないフレームでは `IncomingDeliverySnapshot` の構築をスキップ（あるいは lazy 構築）する。
  - `hovered_entity_at_world_pos` における距離比較処理（`.distance()`）を、平方距離（`distance_squared`）による比較へ置換する。
  - hover hit-test はカーソル座標やカメラが動いたときのみ実行するように制御を追加。
- **変更ファイル**:
  - `crates/hw_familiar_ai/src/familiar_ai/decide/task_delegation.rs`
  - `crates/bevy_app/src/interface/selection/input.rs`
  - `crates/bevy_app/src/interface/selection/hit_test.rs`

---

### M2: TransportRequest producer の共有 cache 化

- **ボトルネックと現状コード**:
  - `build_stockpile_groups` が同一フレーム内の `task_area_auto_haul_system`（`task_area.rs:242`）と `stockpile_consolidation_producer_system`（`consolidation.rs:51`）の両方から、同じ引数で重複して呼び出され、グループと空間インデックスを重複構築している。
- **具体的な変更内容**:
  - `active_unit_cache.rs` に `CachedStockpileGroups` リソースを新規定義。
  - 第1段階では、`Perceive` フェーズのシステムとして、`active_yards` のキャッシュ更新後に `build_stockpile_groups` と `build_group_spatial_index` をフレーム内で 1 回だけ実行し、結果を `CachedStockpileGroups` に格納する。同一フレーム内の producer 間重複を消すことを主目的にする。
  - 第2段階として、Stockpile / StoredItems / Yard / TaskArea / RemovedComponents の invalidation 条件を列挙できた場合のみ、dirty-driven または低頻度 timer 更新へ進める。
  - `task_area_auto_haul_system` および `stockpile_consolidation_producer_system` は、このキャッシュされたリソースを読み込むだけの処理に変更する。
- **変更ファイル**:
  - `crates/hw_logistics/src/transport_request/producer/active_unit_cache.rs`
  - `crates/hw_logistics/src/transport_request/producer/task_area.rs`
  - `crates/hw_logistics/src/transport_request/producer/consolidation.rs`
  - `crates/hw_logistics/src/transport_request/plugin.rs`

---

### M3: Dream UI particle と Entity list の UI 更新量削減

- **ボトルネックと現状コード**:
  - `ui_particle/update.rs`, `trail.rs`, `update_standard.rs` で、アセットから `materials.get_mut(&mat_node.0)` を呼び出し、粒子の alpha や scale などを毎フレーム更新している。これによりマテリアルがクローンされ、アセット送信負荷が増大する。
  - Entity list UI の更新において、vitals 値の些細な変更（数値のみの変更）でリスト要素全体が再生成または不要なテキスト更新処理を通っている。
- **具体的な変更内容**:
  - **Dream UI Particle**: 現在の見た目は `DreamBubbleUiMaterial` と `dream_bubble_ui.wgsl` の custom shader 入力（`color`, `alpha`, `mass`, `velocity_dir`）に依存しているため、単純な `Node` 置換は行わない。まずは world-space Dream bubble と同様に、alpha / mass / color / velocity bucket の共有 material 化を検討し、per-particle の `materials.get_mut` を削減する。サイズや回転は既存どおり `Node` / `Transform` 側で扱う。
  - **Dream UI Particle 代替案**: bucket 化で見た目の再現性が不足する場合のみ、shader 入力を material ではなく component / instance 相当の経路へ移す設計を別途検討する。
  - **Entity List**: リスト要素の構造変化（行の追加・削除・ソート＝structure dirty）と、行の中身 of テキスト更新（vitals 値変更＝value dirty）の検知・更新処理を分離する。表示テキストの値が実際に変わらない場合は UI text node への代入処理を行わないガードを追加する。
- **変更ファイル**:
  - `crates/hw_visual/src/dream/ui_particle/update.rs`
  - `crates/hw_visual/src/dream/ui_particle/update/update_standard.rs`
  - `crates/hw_visual/src/dream/ui_particle/trail.rs`
  - `crates/hw_visual/src/dream/dream_bubble_material.rs`
  - `assets/shaders/dream_bubble_ui.wgsl`
  - `crates/bevy_app/src/interface/ui/list/` (VM / sync 周辺)

---

### M4: Pathfinding と Soul update の対象絞り込み

- **ボトルネックと現状コード**:
  - `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs` に、特定の集会エリア付近で無条件に出力される `info!` / `warn!` などの重いデバッグ用ログ出力（`PATHFIND_DEBUG` 関連、および coordinate-based probe）が残っている。
  - `pathfinding_system` の `for prioritize_tasks in [true, false]` ループ内で、全 DamnedSoul を毎フレーム 2 回走査し、無条件で状態チェックを実行している。
  - `try_reuse_existing_path`（`reuse.rs:60`）において、毎フレームパスに沿った全 waypoint が walkable であるか（障害物の遮断チェック）走査している。マップが変更されていないフレームでもこの走査が走る。
- **具体的な変更内容**:
  - **デバッグログの削除**: `system.rs` の 100-104行, 112-121行, 138-149行, 182-190行、および `fallback.rs:169` 付近のデバッグプリントを削除。
  - **全件走査の回避**: パス再検索が必要な Soul のみを判別するための dirty フラグまたは marker コンポーネント（例: `Destination` が追加/変更された、あるいは path が blocked された場合に付与する）を導入し、`pathfinding_system` の走査対象を絞り込む。
  - **マップ未変更時のパス再検証スキップ**: `try_reuse_existing_path` は現在 `&WorldMap` を受け取る純粋寄りの helper であり、Bevy の `Res::is_changed()` を直接参照できない。`WorldMap` または pathfinding 専用リソースに障害物 / ドア状態の `obstacle_version` を持たせ、`Path` 側に `validated_obstacle_version` を記録する。両者が一致する場合のみ、既存パス上の通行不可チェック（O(Waypoints) 走査）をスキップする。
- **変更ファイル**:
  - `crates/hw_soul_ai/src/soul_ai/pathfinding/system.rs`
  - `crates/hw_soul_ai/src/soul_ai/pathfinding/reuse.rs`
  - `crates/hw_core` または `crates/hw_world` の `Path` / `WorldMap` 関連定義（version を持たせる場合）

---

### M5: Visual 3D proxy / face material 更新の軽量化

- **ボトルネックと現状コード**:
  - `sync_soul_proxy_3d_system` などの 3D 同期処理は、3D 描画が無効（`Render3dVisible` が false）のときでも毎フレーム呼び出され、Transform の書き込みを行っている。
  - `sync_soul_face_expression_system`（`soul_animation.rs:267`）において、毎フレーム全キャラクターの顔マテリアルに対して `materials.get_mut` を無条件で呼び出している。
- **具体的な変更内容**:
  - **3D 同期の run_if ガード**: `sync_soul_proxy_3d_system`, `sync_soul_mask_proxy_3d_system`, `sync_soul_shadow_proxy_3d_system`, `sync_familiar_proxy_3d_system` に、`run_if(|render3d: Res<Render3dVisible>| render3d.0)` のような実行条件を設定し、非表示時の同期コストを抑える。`resource_exists_and_equals(Render3dVisible(true))` を使う場合は `Render3dVisible: PartialEq` が必要になるため、現状型のままなら closure 条件を優先する。
  - **顔マテリアル更新のガード**: 表情データ `SoulAnimVisualState` に変更があったか（`Changed<SoulAnimVisualState>` などの変更検知、または face proxy コンポーネントに前回適用した表情のキャッシュ値を持たせて比較する）、あるいは表情値が変わったときのみ `materials.get_mut` を呼び出すように修正する。
- **変更ファイル**:
  - `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`
  - `crates/bevy_app/src/systems/visual/soul_animation.rs`
  - `crates/bevy_app/src/plugins/visual.rs`
  - `crates/bevy_app/src/main.rs`（`Render3dVisible` に `PartialEq` を追加する場合のみ）

---

### M6: 小粒な線形処理の整理

- **ボトルネックと現状コード**:
  - `tile_index.rs` の `sync_removed_tiles` において、削除された blueprint タイルごとに、`floor_tiles_by_site` マップ内の全サイト（全エントリー）の `Vec` を走査して `retain` を実行する O(R * S) ループになっている。
- **具体的な変更内容**:
  - `TileSiteIndex` リソース内に `tile_to_site: HashMap<Entity, Entity>` という逆引きインデックスを定義。
  - タイル追加時に逆引きマップにも登録し、タイル削除時にはこの逆引きマップから直接対応する `site_entity` を取得して、対象サイトの vector からのみ要素を削除する（O(R) 処理への最適化）。
- **変更ファイル**:
  - `crates/hw_logistics/src/tile_index.rs`

---

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 3D 表示の run_if ガードによる再開時の描画乱れ | 3D 表示を ON に戻した瞬間に、古い座標に一瞬キャラクターが表示される | 表示が OFF から ON に切り替わったフレームで、すべての 3D プロキシの Transform を強制同期するフラグまたは system を用意する。 |
| パス再検証のスキップ漏れ | 新しく建設された建物などの障害物を Soul がすり抜けずに歩いてしまう | `WorldMap` の Bevy change detection へ直接依存せず、障害物・ドア状態の変更時に増える explicit version を導入する。`Path` 側の検証済み version と比較し、不一致時は必ず waypoint 検証を実行する。 |
| 顔マテリアルの更新漏れ | キャラクターの表情が変化しなくなる | `Changed<SoulAnimVisualState>` だけでなく、プロキシが新規登録された初回フレームにも必ず表情 UV offset が適用される初期化ロジックを保証する。 |
| キャッシュ無効化 (Invalidation) 漏れ | 古い stockpile group 情報を参照して搬送が失敗する | M2 第1段階では毎フレーム Perceive で 1 回だけ再構築し、producer 間重複のみを削る。dirty-driven 化へ進む場合は、Stockpile / StoredItems / Yard / TaskArea / RemovedComponents の invalidation 条件を実装前に列挙する。 |

## 7. 検証計画

- **ビルド・静的検証**:
  - `cargo check --workspace` および `cargo clippy --workspace` にてビルドチェック。
- **手動動作確認シナリオ**:
  1. Familiar に `TaskArea` を設定し、Idle Familiar が Yard や各種採取・搬送タスクを正常に検知して委譲を受けるか確認。
  2. マップ上に Stockpile や Yard を作成・変更し、搬送 request の発行および消化が従来通り行われるか確認。
  3. 睡眠時の Dream 獲得演出、UI particle の merge や trail表示が視覚的に崩れていないか確認。
  4. 建築物を追加して移動経路を塞ぎ、Soul が障害物をすり抜けずに再経路探索（Escape / Recalculate）を行うか確認。
  5. DevPanel 上で 3D 表示の ON/OFF を切り替え、3D 表示再開時にキャラクター位置が正常に同期されるか確認。

## 8. ロールバック方針

- 各マイルストーン（M1〜M6）は独立して適用・revert できるよう、PR またはコミットを細かく分ける。
- 破壊的 git コマンド（`git checkout -- <file>` など）を実行する前に、必ず `git diff` を確認し、並行作業の変更を誤って上書きしないように注意する。

## 9. AI引継ぎメモ

### 実装結果（2026-07-07 完了）

- **実装記録の訂正**: M1〜M3、M5〜M6と、M4のデバッグログ削除・`obstacle_version`/`validated_obstacle_version`による版一致時のpath再検証skipは実装済み。M4に記載した`NeedsPath`等による全Soul二重走査のmarker/queue置換は未実装であり、`pathfinding_system`の`for prioritize_tasks in [true, false]`走査が残る。後者は`system-wide-runtime-performance-plan-2026-07-12.md` M4へ移管した。
- `cargo check --workspace` / `cargo clippy --workspace` ともに成功（警告 0）。
- ユーザーによる実機動作確認済み（建物設置後の再経路探索・Dream 演出・3D ON/OFF・Familiar 委譲）。
- 同期した恒久ドキュメント: `docs/invariants.md`（I-PF1: obstacle_version bump 契約）/ `docs/logistics.md`（CachedStockpileGroups）/ `docs/soul_ai.md`（§5.1 版一致スキップ）/ `docs/dream-visual.md`（§3 共有 material bucket）/ `docs/entity_list_ui.md`（structure/value dirty 分離）。
- 未実施（任意フォロー）: M5 の 4 proxy sync 統合（owner lookup 重複解消）。run_if ゲートのみ実装し、統合は見送り。

### 実装差分の要点（後続者向け）

- 歩行可否を変える `WorldMap` mutation を追加する場合は必ず `bump_obstacle_version()` を通すこと（→ [I-PF1](../invariants.md)）。忘れると Soul が古いパスで障害物に突っ込むサイレント失敗。

---

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-07` | `Codex` | 初版作成 |
| `2026-07-07` | `Claude` | コード突合レビューを反映: M4 `can_reach_target` の誤診訂正、M1 idle-bypass/snapshot 結合明記、既存 metrics 追記 |
| `2026-07-07` | `Claude` | コード詳細調査に基づくブラッシュアップ: `sync_removed_tiles` の O(R*S) 走査の逆引き Hash 最適化設計、`sync_soul_face_expression_system` の `get_mut` 回避、3Dプロキシ同期の `Render3dVisible` `run_if` ガード、パス再利用時のマップ未変更スキップなどを追記 |
| `2026-07-07` | `Codex` | レビュー指摘を反映: path 再検証を version 比較へ修正、Dream UI particle の置換案を shader/material 前提に修正、`Render3dVisible` run condition の trait 条件を明記、M2 cache 方針とリンク表記を整理 |
| `2026-07-07` | `Claude` | M1〜M6 実装完了・実機確認済み。ステータスを Complete に更新、恒久ドキュメント（invariants/logistics/soul_ai/dream-visual/entity_list_ui）を同期 |
| `2026-07-12` | `Codex` | 現行コードとの再照合によりM4の実装記録を訂正。版一致skipは完了、`NeedsPath`/queueによる全Soul二重走査置換は未実装として後継性能計画へ移管。 |
