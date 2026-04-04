# Outdoor Lamp 照明影実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `lighting-visual-plan-2026-04-04` |
| ステータス | `Draft` |
| 作成日 | `2026-04-04` |
| 最終更新日 | `2026-04-04` |
| 作成者 | `Codex` |
| レビュー | `Claude Sonnet 4.6 (2026-04-04)` / `Cursor 追記 (2026-04-04)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  現状の Outdoor Lamp は `PoweredVisualState` による白/灰の色切り替えしかなく、ランプ照明によって `Soul` や建物に落ちる影は存在しない。
- 到達したい状態:
  通電した Outdoor Lamp が近傍の `Soul` と建物にローカルな影を落とし、停電時はその影が消える。
- 成功指標:
  1 基のランプを置いただけで、近くを歩く Soul や壁際の建物に「ランプ由来の影」が視認できる。

## 2. スコープ

### 対象（In Scope）

- `OutdoorLamp` のローカル照明による影
- `Soul` と建物を対象にした shadow caster / receiver 経路
- 既存の 3D RtT shadow 経路の再利用
- `PoweredVisualState` とランプ light の on/off 同期

### 非対象（Out of Scope）

- Room 接続や Indoor lighting のゲームロジック
- 2D foreground 全体への局所照明
- 画面全体の昼夜・露出・ポストプロセス
- `Room Light` や電力網のゲームロジック拡張

## 3. 現状とギャップ

### 現状（コードベース確認済み）

- `OutdoorLamp` は `post_process.rs::setup_outdoor_lamp` で `PowerConsumer { demand: OUTDOOR_LAMP_DEMAND }` のみ挿入。ランプ専用の light entity はない。
- `spawn.rs` では `BuildingType::OutdoorLamp` が **専用 arm ではなく**、`SandPile | BonePile | WheelbarrowParking | OutdoorLamp` の共通 arm に含まれる。3D では `equipment_1x1_mesh` + `Transform::from_xyz(..., TILE_SIZE * 0.3, ...)` + `building_3d_render_layers()` = `[LAYER_3D, LAYER_3D_SHADOW_RECEIVER]` で `Building3dVisual { owner }` を作るが、子 light は持たない。
- 3D RtT 側には `DirectionalLight`（`shadows_enabled: true`, `illuminance: 12_000.0`, cascade `first_cascade_far_bound: 120.0 / maximum_distance: 500.0`）が 1 本あり、`RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW])` でランプ影もカバーできる構成。
- `SoulShadowProxy3d` は `[LAYER_3D, LAYER_3D_SOUL_SHADOW]` で spawn 済み（`damned_soul/spawn.rs`）。
- `PoweredVisualState` の on/off は `hw_jobs/src/visual_sync/observers.rs` の 3 つの Observer が管理。`hw_visual/src/power.rs` の `sync_powered_visual_system` は Sprite color のみ同期し、lamp light の `Visibility` は未対応。
- `cleanup_building_3d_visuals_system`（`building3d_cleanup.rs`）は `Building3dVisual.owner == removed_entity` で全該当 entity を `despawn()` する。

### 問題

- `PoweredVisualState` が light に伝わらないため、停電時も光源が消えない（光源自体がまだないが）。
- visible Soul mesh は `NotShadowCaster` であり、lamp light を追加しても visible mesh では影が出ない（`SoulShadowProxy3d` 経由が必須）。

### 本計画で埋めるギャップ

既存の 3D RtT shadow パイプラインを崩さず、Outdoor Lamp の `Building3dVisual` エンティティの子として local light を spawn し、`PoweredVisualState` と同期する。

## 4. 実装方針（高レベル）

- 影は 2D フェイクではなく、3D RtT 側の実ライト shadow で出す。対象は `Soul` と建物に限定する。
- **spawn 設計**: Outdoor Lamp ごとに `Building3dVisual` 子エンティティとして local light を spawn する。子なので `Building3dVisual` を `despawn()` すれば自動クリーンアップされ、専用クリーンアップシステムが不要。
- **RenderLayers**: lamp local light に `RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW])` を付与する。これは現行 `DirectionalLight` と同じ設定で、建物（`LAYER_3D_SHADOW_RECEIVER`）と Soul shadow proxy（`LAYER_3D_SOUL_SHADOW`）の両方に影が出る。
- **light 種別**: 初手は `SpotLight` を使用する。`PointLight` は cubemap shadow と `PointLightShadowMap` 管理が必要でコストも重くなりやすい。`SpotLight` は cone を下向きに絞って影範囲を制御しやすい。`shadows_enabled: true`、`range` は `TILE_SIZE * 6.0` 程度、`outer_angle` は狭めに始める。**M1 では Bevy 0.18 の `SpotLight` が RtT で影を落とすことを実行と一次情報（docs.rs / `~/.cargo/registry`）で確認してから本番前提にする。Directional と同一シャドウ設定の「流用」と決め打ちしない。**
- **owner 解決**: lamp light child には spawn 時点で `OutdoorLampLight3d { owner: building_entity }` を必ず付与する。これにより M2 の同期は `PoweredVisualState` を持つ building owner と light child を `owner` で直接突き合わせられる。
- **on/off 同期**: `PoweredVisualState` が変化したとき、lamp light の `Visibility` を `Visible` / `Hidden` に切り替えるシステムを追加する。初期値は `Visibility::Hidden`（`PoweredVisualState` 初期値が `is_powered: false` のため）。
- Soul の影は `SoulShadowProxy3d` を caster として再利用。visible mesh 側の `NotShadowCaster` 方針は維持する。
- ランプの glow は補助表現として残してよいが、本計画の主成果物ではない。

### 設計上の前提

- この影は 3D RtT に参加している `Soul`（`SoulShadowProxy3d`）/ 建物（`Building3dVisual`）にのみ効く。2D foreground 専用 Sprite には効かない。
- `RenderLayers` は camera / light / mesh が交差しないと効かない。lamp light の layer は必ず `[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]` の 3 つを含める。
- `DirectionalLightShadowMap { size: 4096 }` はグローバルリソース。Spot / Point など追加ライトの shadow が **別リソース・別パス**になる可能性があるため、動作確認前に推測で「Directional と同一設定の流用」と決め打ちしない。

### Bevy 0.18 API での注意点

- `SpotLight` の `shadows_enabled` はデフォルト `false`。明示的に `true` にする。
- `range` を大きくしすぎると shadow map 解像度が落ちる。ランプの照明半径に合わせて `TILE_SIZE * 6.0` 程度から始める。
- `SpotLight` は direction を `Transform::looking_at(...)` で決める必要がある。真下固定で始め、必要なら少し斜め前へ振って影を見やすくする。
- shadow 付き local light はフレームコストが高い。同時有効数が増えた場合は近傍ランプだけ有効化するフォールバックを検討する。

## 5. マイルストーン

### M1: Outdoor Lamp の Building3dVisual 子として local SpotLight を追加する

- 変更内容:
  `spawn.rs` で **OutdoorLamp だけ** `Building3dVisual` の子として `SpotLight` を spawn する。現状は他建物と共通の match arm のため、**(a) `BuildingType::OutdoorLamp` 専用 arm に分離して** `commands.spawn((..., Building3dVisual { owner })).with_children(|p| { ... })` とする、または **(b)** 共通 spawn 後に `kind == OutdoorLamp` のときだけ `commands.entity(building3d_entity).with_children(...)` を呼ぶ、のいずれかとする。`Visibility::Hidden` で初期化し、`RenderLayers::from_layers(&[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW])` と `OutdoorLampLight3d { owner: building_entity }` を同時付与する。
- **メッシュ位置との関係**: 親 `Building3dVisual` の根は現状 `TILE_SIZE * 0.3` 高さ。ライトは子の **ローカル** `Transform` で `y ≈ TILE_SIZE * 0.8`（ポール先）などを指定し、見た目と照らし合わせる。M3 で専用 mesh に差し替える場合はライト子のオフセットを再調整する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
    - OutdoorLamp 用に `with_children` を追加し、`p.spawn((SpotLight { shadows_enabled: true, range: TILE_SIZE * 6.0, ... }, OutdoorLampLight3d { owner }, ...))` を子として追加
    - 子 `SpotLight` の `Transform::looking_at(...)` で下向き cone（例: ローカルで `looking_at` し、ワールド整合は親 Transform と合成される）
    - RenderLayers は `[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]`（DirectionalLight と同じ）
  - `crates/hw_visual/src/visual3d.rs`
    - `OutdoorLampLight3d { owner: Entity }` マーカーコンポーネントを追加
- クリーンアップについて:
  子エンティティのため `cleanup_building_3d_visuals_system` で `Building3dVisual` を `despawn()` すると自動削除される。専用クリーンアップシステムは不要。
- 完了条件:
  - [ ] `OutdoorLamp` ごとに light 子エンティティが生成される（初期は Visibility::Hidden）
  - [ ] light 子エンティティが `OutdoorLampLight3d { owner }` を持つ
  - [ ] building 削除時に light も自動的にクリーンアップされる
  - [ ] `cargo run` で SpotLight の影が RtT 上に出る（出ない場合は Bevy 0.18 の shadow 設定を一次情報で確認してから続行）
- 検証:
  - `cargo check`
  - `cargo run`（上記完了条件の手動確認）

### M2: 通電状態と lamp light Visibility の同期を実装する

- 変更内容:
  `PoweredVisualState` と `OutdoorLampLight3d.owner`（= building entity）を対応づけ、`Visibility` を `Visible` / `Hidden` に切り替えるシステムを追加する。既存の `sync_powered_visual_system`（Sprite color 同期）と並列稼働させる。
- **クエリ方針（hw_visual の crate 境界を崩さない）**:
  - **推奨**: `Query<(&OutdoorLampLight3d, &mut Visibility)>` で全ランプ light を走査し、各 `owner` に対して `PoweredVisualState::is_powered` を読んで `Visibility` を更新する。`Building` / `BuildingType` は不要（`hw_jobs` への新規依存を増やさない）。
  - **最適化（任意）**: `Query<(Entity, &PoweredVisualState), Changed<PoweredVisualState>>` で変化した building のみ処理し、同じ `owner` を持つ `OutdoorLampLight3d` の `Visibility` を更新する。child 走査は不要（owner で light を引く）。
  - 「`Changed` を light 側に付ける」ことは owner と別 entity のためそのままでは使えない点に注意。
- 変更ファイル:
  - `crates/hw_visual/src/power.rs`
    - `sync_lamp_light_visibility_system` を追加（上記クエリ方針のいずれか）
  - `crates/hw_visual/src/visual3d.rs`
    - M1 で追加した `OutdoorLampLight3d` を再利用
  - `crates/bevy_app/src/plugins/visual.rs`
    - `sync_lamp_light_visibility_system` を `Visual` スケジュールに登録
- 完了条件:
  - [ ] 通電ランプ近傍の Soul にローカル shadow が出る
  - [ ] `Unpowered` 付与時に lamp shadow が消える（`Visibility::Hidden`）
  - [ ] visible Soul mesh の shadow 設計（`NotShadowCaster`）を壊さない
- 検証:
  - `cargo check`
  - `cargo run`（手動: 1 基建設→通電→Soul 近傍で影確認→停電で消えることを確認）

### M3: 建物影・ドキュメント整合を整える

- 変更内容:
  建物が lamp light の receiver として自然に見えるかを確認し、影品質が問題なら `OutdoorLamp` 専用 mesh の改善を検討する。light / caster / receiver の layer 構成と lifecycle をドキュメントに残す。
- 変更ファイル:
  - `docs/soul_energy.md`
    - lamp local light の lifecycle（spawn / powered sync / despawn 経路）を記載
  - `docs/building.md`
    - OutdoorLamp エンティティ構造（Building → Building3dVisual → OutdoorLampLight3d 子）を記載
  - `docs/architecture.md`
    - 3D RtT shadow pipeline の caster / receiver / light の layer 対応表に lamp local light を追記
  - （任意）`crates/bevy_app/src/plugins/startup/visual_handles.rs`
    - OutdoorLamp 専用 mesh が必要な場合のみ追加
- 完了条件:
  - [ ] ランプ照明で建物にも影が落ちる
  - [ ] 影が出る entity（caster / receiver / light）と RenderLayers の対応が docs に残る
  - [ ] Room Light 追加時の拡張ポイント（`OutdoorLampLight3d` パターンの再利用）が明文化される
- 検証:
  - `cargo check`
  - `cargo run`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| shadow 付き local light をランプごとに増やしすぎる | GPU コスト増大 | 初手は `SpotLight` + 狭い `outer_angle` + `range: TILE_SIZE * 6.0`、同時有効数が厳しい場合は近傍ランプだけ有効化を検討 |
| light の RenderLayers が `[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]` でない | 建物 or Soul のどちらかの影が出ない | DirectionalLight と完全同一の layer 設定を固定しコメントで明記する |
| Soul visible mesh に誤って caster を戻す | 影の二重化や見た目破綻 | `SoulShadowProxy3d` のみ caster、visible 側 `NotShadowCaster` 維持 |
| `Visibility::Hidden` 初期化を忘れる | 停電中もランプが光る | spawn 時に `Visibility::Hidden` を明示的に付与し、同期システムで上書きする設計にする |
| owner 解決を child 走査に依存する | 同期コードが複雑化し cleanup 後に壊れやすい | `OutdoorLampLight3d { owner }` を spawn 時点で必須化し、owner 直接照合にする |
| 1x1 equipment の箱形状で影が不自然 | ランプらしさが出ない | 影品質が問題になった時点で OutdoorLamp 専用 mesh を追加（M3 の任意作業） |

## 7. 検証計画

- 必須:
  - `cargo check`
- 手動確認シナリオ:
  - ランプ 1 基を Yard 内に建設し、停電時は local shadow が出ないことを確認
  - Soul Spa 稼働後に通電し、ランプ近傍の Soul に影が出ることを確認
  - ランプの近くに wall / 1x1 equipment を置き、建物に影が出ることを確認
  - 需要超過で `Unpowered` に戻った瞬間、lamp light と影が消えることを確認
  - 通常視点と矢視モードの両方で影の方向と receiver が破綻しないことを確認
- パフォーマンス確認（必要時）:
  - ランプ数を増やした状態で shadow 付き light 増加によるフレーム低下を確認する
- （任意）検証の手早さ:
  - Soul Spa まで進まず通電だけ試す場合は、開発ビルドで電源を確実に足す既存手段（デバッグ UI・チート・セーブ改変など）があればここに追記する。無ければ従来シナリオ（Soul Spa 稼働後に通電）でよい。

## 8. ロールバック方針

- どの単位で戻せるか:
  `OutdoorLamp` の local light 生成、powered 同期、専用 mesh 改善を別々に戻せる。
- 戻す時の手順:
  まず lamp local light の spawn と sync system を外し、必要なら補助 glow だけ残す。`PoweredVisualState` 自体は既存 UI/色変化に使うので残す。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
  - なし
- 未着手/進行中:
  - M1, M2, M3

### 次のAIが最初にやること

1. `spawn.rs` で OutdoorLamp が `SandPile | … | OutdoorLamp` の**共通 arm**にあることを踏まえ、OutdoorLamp 専用に分離するか spawn 直後に `with_children` するなどして、`Building3dVisual` の子として `SpotLight` を追加する（`shadows_enabled: true`, `range: TILE_SIZE * 6.0`, `Visibility::Hidden`, RenderLayers: `[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]`）。親メッシュは `TILE_SIZE * 0.3`、ライト子はローカル y でポール高（例 `TILE_SIZE * 0.8`）を調整。
2. `hw_visual/src/visual3d.rs` に `OutdoorLampLight3d { owner: Entity }` マーカーを追加し、spawn 時に同時付与する。
3. `hw_visual/src/power.rs` に `sync_lamp_light_visibility_system` を追加し、M2 のクエリ方針（`OutdoorLampLight3d.owner` と `PoweredVisualState` の照合、child 走査なし）に従う。
4. `cargo check` → `cargo run` で Spot の影と通電同期を手動確認する。

### 既存コードの要点（実装前に確認）

| ファイル | 要点 |
| --- | --- |
| `spawn.rs` L195-250頃 | `SandPile \| … \| OutdoorLamp` 共通 arm。OutdoorLamp だけ分離して `with_children` するか、spawn id を取って後から子追加 |
| `startup_systems.rs` L120-140 | DirectionalLight の RenderLayers 設定（lamp light と同じ層にする根拠）|
| `building3d_cleanup.rs` | `Building3dVisual.despawn()` → 子エンティティ自動削除の確認 |
| `observers.rs` (hw_jobs) | `PoweredVisualState` の初期値と更新タイミング（on_unpowered_added/removed）|
| `power.rs` (hw_visual) | `sync_powered_visual_system`（Sprite color のみ）→ lamp light 同期は別関数で追加 |
| `render.rs` (hw_core) | `LAYER_3D_SHADOW_RECEIVER = 5`, `LAYER_3D_SOUL_SHADOW = 4` の定数値確認 |

### ブロッカー/注意点

- `Building3dVisual` の子として spawn すれば `cleanup_building_3d_visuals_system` で自動削除される（`despawn()` は再帰的に子も削除する Bevy のデフォルト動作）。専用クリーンアップシステムは不要。
- lamp light の RenderLayers を `[LAYER_3D, LAYER_3D_SHADOW_RECEIVER, LAYER_3D_SOUL_SHADOW]` にしないと建物か Soul どちらかの影が出ない。DirectionalLight のコードコメントを参照のこと。
- `PoweredVisualState` 初期値は `is_powered: false`（`on_power_consumer_visual_added` 参照）。lamp light も `Visibility::Hidden` で spawn し、同期システムで `Visible` に切り替える設計にする。
- owner 解決は `Building3dVisual` の child 走査に頼らず、`OutdoorLampLight3d { owner }` で直接結ぶ。
- visible Soul mesh は `NotShadowCaster`。lamp light の影を出したい場合は `SoulShadowProxy3d`（`[LAYER_3D, LAYER_3D_SOUL_SHADOW]`）が必要。
- lamp shadow は 3D RtT 参加物にしか効かない。2D foreground にも効かせたい場合は別設計になる。
- 既存ワークツリーに `docs/plans/3d-rtt/archived/wfc-ms2-5-terrain-zone-mask.md` の未コミット変更があるため、触らない。
- `spawn.rs` の OutdoorLamp は他建物と **共通 arm** のため、実装時は M1 の (a)(b) のどちらで子ライトを足すか先に決める。

### 参照必須ファイル

- `crates/bevy_app/src/systems/jobs/building_completion/spawn.rs`
- `crates/bevy_app/src/plugins/startup/startup_systems.rs`
- `crates/bevy_app/src/systems/visual/building3d_cleanup.rs`
- `crates/hw_visual/src/power.rs`
- `crates/hw_visual/src/visual3d.rs`
- `crates/hw_core/src/constants/render.rs`
- `crates/hw_jobs/src/visual_sync/observers.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `docs/soul_energy.md`
- `docs/building.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-04-04` / `not run`
- 未解決エラー:
  - なし

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-04` | `Codex` | 初版作成 |
| `2026-04-04` | `Codex` | 焦点を glow 表現から Outdoor Lamp のローカル shadow 実装へ更新 |
| `2026-04-04` | `Claude Sonnet 4.6` | コードベース調査に基づき全節をブラッシュアップ：spawn設計（Building3dVisual の子エンティティ化）、RenderLayers 具体値の明記、M1-M3 の変更ファイルと実装詳細の精緻化、M4をM3に統合、AI引継ぎメモに実装要点テーブル追加 |
| `2026-04-04` | `Cursor` | レビュー反映：`spawn.rs` 共通 match arm の明記、メッシュ高とライト子のオフセット、Spot shadow の一次情報確認、M2 のクエリ方針（`hw_visual` 境界）、見出し `### M1-M3`、検証・引継ぎの追記、更新履歴の誤字修正 |
