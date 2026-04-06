# ワールドマップ LOD 実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `world-map-lod-strategy-2026-04-06` |
| ステータス | `Draft` |
| 作成日 | `2026-04-06` |
| 最終更新日 | `2026-04-06` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |
| 関連ドキュメント | `docs/world_layout.md`, `docs/architecture.md`, `docs/art-style-criteria.md` §3.5, `docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md` §8.3 |

## 1. 目的

- 解決したい課題:
  - 地形は `spawn_map` で **100 x 100 = 10,000 タイル**を個別 `Mesh3d` として生成している。
  - `TerrainSurfaceMaterial` は `terrain_id_map` 近傍参照、macro noise、overlay、river flow を持ち、**遠景でも近景と同じシェーダ負荷**を払う。
- 到達したい状態:
  - **LOD は描画専用**として導入し、`WorldMap` / pathfinding / 生成データには触れずに、ズームアウト時だけ描画コストを段階的に落とす。
  - TopDown の遠景では、10,000 タイルをそのまま描くのではなく、**少数 draw に集約した遠景表現**へ切り替える。
  - 境界リボンは **遠景でも残す**。遠景ではむしろ地形の読みやすさを支える主役として扱い、LOD の削減対象は地形本体の detail に寄せる。
  - 矢視・Section 系は correctness 優先で、遠景用の単純化を無理に適用しない。
- 成功指標:
  - TopDown の遠景ズームで、地形描画の entity / draw / fragment cost を近景より明確に削減できる。
  - LOD 切替で目立つポッピングや 1 フレーム明滅が発生しない。
  - `TerrainChangedEvent` 後も far 表示が stale にならない。

## 2. スコープ

### 対象（In Scope）

- TopDown 地形描画の LOD 戦略策定
- LOD 判定に使う **画面内タイルサイズ**または等価の screen-space metric 導入
- 地形タイル本体 (`spawn_map` / `TerrainSurfaceMaterial`) の near / mid / far 切替
- 境界リボン (`boundary.rs`) の LOD 連動表示と、far で残すための簡略化方針
- `TerrainChangedEvent` と far 表示の同期経路
- `docs/world_layout.md` / `docs/architecture.md` へ反映する前提整理

### 非対象（Out of Scope）

- `WorldMap` の解像度変更
- pathfinding / logistics / AI の探索粒度変更
- worldgen アルゴリズムの変更
- 建物 / Soul / Familiar の LOD 本体
- ミニマップ専用 UI の設計

## 3. 現状とギャップ

### 現状

- `crates/bevy_app/src/world/map/spawn.rs`
  - 全セルに `Tile + Mesh3d(tile_mesh) + TerrainSurfaceMaterial + Transform` を生成する。
- `crates/bevy_app/src/world/map/boundary.rs`
  - worldgen 由来の境界をポリライン化し、地形とは別メッシュ群としてスポーンする。
- `crates/bevy_app/src/systems/visual/camera_sync.rs`
  - `MainCamera` の `transform.scale.x` を RtT 用 `Camera3d` の `OrthographicProjection.scale` へ毎フレームコピーする。
- `crates/hw_core/src/quality.rs`
  - 現在の品質設定は **RtT 解像度係数のみ**で、地形そのものの描画粒度は変わらない。

### 問題

- 既存の品質設定は「描画先テクスチャの解像度」を下げるだけで、**地形の描画仕事量そのもの**を減らさない。
- LOD 判定に必要な **ズーム契約**が未固定で、`PanCamera::default()` 任せになっている。
- `TerrainChangedEvent` は `terrain_id_map` 更新には使えているが、将来 far 表示を追加したときの同期先がまだない。
- 矢視は `SectionCut` と地形 3D 表現の正しさが優先であり、TopDown と同じ far 表示を流用しにくい。
- 境界リボンは遠景での視認性が高く、単純な非表示は見た目の主情報を消してしまう。

### 本計画で埋めるギャップ

- `ortho.scale` ではなく **screen-space のタイル見かけサイズ**を LOD 指標として定義する。
- LOD を 3 段階に分け、**近景は現行維持、中景は shader 簡略化、遠景は TopDown 限定の overview impostor** を採用する。
- 境界リボンは全 LOD で残し、必要なら **far 専用の簡略リボン**へ切り替える。
- `TerrainChangedEvent` を far 表示にも接続し、runtime 地形変化と整合させる。

## 4. 実装方針（高レベル）

### 4.1 採用方針

- **LOD0（近景）**
  - 現行維持。
  - 10,000 タイル + `TerrainSurfaceMaterial` + 境界リボンをそのまま使用する。
- **LOD1（中景）**
  - 地形タイル entity は維持しつつ、境界リボンは維持する。
  - `TerrainSurfaceMaterial` は **簡略 variant** を持たせ、macro overlay / river detail / 近傍ブレンド範囲の一部を落とす。
  - 必要なら境界リボン側だけ **sample 密度 / マテリアル detail / 幅**を落とした中景 variant を使う。
  - 目的は「構造を変えずに fragment cost を先に下げつつ、遠景の輪郭情報を保つ」こと。
- **LOD2（遠景, TopDown 限定）**
  - TopDown のみ、地形を **1 枚または極少数の overview mesh** に切り替える。
  - overview は `TerrainIdMap` と `TerrainFeatureMap` から生成する **far 専用の baked / semi-baked image** を貼る。
  - このとき通常の 10,000 タイル entity は `Visibility` で無効化するが、境界リボンは **残す**。必要なら far 用に簡略化した別リボンへ切り替える。

### 4.2 なぜ chunk 先行ではなく overview impostor を先に取るか

- マップは **固定 100x100** で、far では「各セルの精密な形」より「川・砂・草土の大局」が読めれば十分。
- chunk 化は部分更新・境界整合・SectionCut との関係が重く、far 専用 1 枚表現の方が **導入コストに対する削減幅が大きい**。
- 既存の `TerrainChangedEvent` はピクセル更新モデルと相性が良く、overview image の部分更新へ自然に拡張できる。

### 4.3 LOD の駆動変数

- camera distance ではなく、**1 タイルが画面上で何 px に見えるか**を使う。
- 理由:
  - RtT 解像度変更 (`QualitySettings.rtt_scale`) が入っても意味が崩れにくい。
  - TopDown / 矢視で同じ `scale` でも見え方が違うため、screen-space 基準の方が扱いやすい。

### 4.4 モード制約

- `ElevationDirection::TopDown`
  - `LOD0 / LOD1 / LOD2` の全段を許可する。
- `ElevationDirection::{North, South, East, West}`
  - `LOD0 / LOD1` まで。
  - `LOD2` は禁止し、overview impostor を使わない。

### 4.5 Bevy 0.18 API での注意点

- `Camera3d` のズームは `OrthographicProjection.scale` が正本で、`MainCamera` 側の `Transform.scale` と毎フレーム同期されている。
- LOD 切替は material の全面差し替えではなく、**Resource に集約した現在 LOD 状態**と `Visibility` / shader flag の更新で扱う。
- shader variant を使う場合は、既存 `TerrainSurfaceMaterial` の bind group 構成を壊さず、`ExtendedMaterial` 側の定数分岐で済む範囲を優先する。

## 5. LOD レベル定義

| LOD | 適用条件 | 地形本体 | 境界リボン | 備考 |
| --- | --- | --- | --- | --- |
| `LOD0` | タイルが十分大きい | 現行 `TerrainSurfaceMaterial` | 表示 | 近景・通常プレイ |
| `LOD1` | タイルが中サイズ | タイル entity は維持、shader 簡略化 | 表示（必要なら簡略 variant） | まず地形本体の負荷を削る |
| `LOD2` | TopDown かつタイルが極小 | overview impostor | 表示（far 用簡略 variant 可） | far 専用 |

**閾値の決め方**

1. 実測で `tile_screen_px` を取る。
2. `LOD0 -> LOD1` と `LOD1 -> LOD2` に別の enter / exit 値を持たせ、ヒステリシスを入れる。
3. 先に閾値を固定してから shader / visibility 切替を入れる。順序を逆にしない。

## 6. 実装オプション比較

### 6.1 採用: TopDown far overview impostor

- 内容:
  - map 全体と同じ world footprint を持つ quad または少数 mesh を 3D 側に置く。
  - far 専用 image を貼り、TopDown 遠景では通常タイルを隠す。
  - 境界リボンは別レイヤーとして残し、overview の上に重ねる。
- 利点:
  - draw 数と fragment cost の削減幅が最も大きい。
  - `TerrainChangedEvent` をピクセル更新へ再利用しやすい。
  - `WorldMap` と切り離しやすい。
- 欠点:
  - 矢視には使えない。
  - 近景へ戻る閾値の設計が必要。
  - overview 側の色と境界リボンの見え方が喧嘩しないよう、far 用トーン合わせが必要。

### 6.2 併用: shader 簡略 LOD1

- 内容:
  - 現行タイル構造は維持し、遠景寄りでは詳細効果だけ落とす。
- 利点:
  - 導入しやすい。
  - 矢視でも適用しやすい。
- 欠点:
  - entity 数は減らない。

### 6.3 併用: 境界リボンの専用 far simplification

- 内容:
  - 境界リボンは非表示にせず、far では `Catmull-Rom` のサンプル密度、ラウンドキャップ、マテリアル detail を落とした専用表現へ切り替える。
- 利点:
  - 遠景で必要な輪郭情報を残せる。
  - 地形本体だけ impostor 化しても、読みやすさを維持しやすい。
- 欠点:
  - 境界側にも LOD 分岐が増える。

### 6.4 先送り: chunk mesh 化

- 内容:
  - 8x8 / 16x16 などで地形をまとめる。
- 位置づけ:
  - `LOD1` と `LOD2` を入れても draw/extract cost が残る場合の第 2 段。
- 先送り理由:
  - 現時点では far overview の方が費用対効果が高い。
  - 部分更新と境界整合が重い。

## 7. マイルストーン

## M1: LOD 観測基盤とズーム契約の固定

- 変更内容:
  - `tile_screen_px` を導出する helper / Resource を追加する。
  - `TopDown` / 矢視で使う LOD 許可範囲を明文化する。
  - デバッグ表示またはログでズームレンジを記録し、閾値案を固定する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/visual/camera_sync.rs`
  - `crates/bevy_app/src/plugins/visual.rs`
  - `docs/world_layout.md`
  - `docs/architecture.md`
- 完了条件:
  - [ ] `tile_screen_px` の算出が 1 箇所に定義されている
  - [ ] LOD enter / exit 閾値の初期値が決まっている
  - [ ] TopDown と矢視の適用差が docs に書かれている
- 検証:
  - `cargo check --workspace`
  - TopDown / 矢視で zoom in / out して LOD state が想定どおり変わることを確認

## M2: LOD1 導入（中景の簡略化）

- 変更内容:
  - `TerrainSurfaceMaterial` に LOD1 用の簡略パスを追加する。
  - 境界リボンは維持しつつ、必要なら中景用 simplification を追加する。
  - 近景復帰時のヒステリシスを入れる。
- 変更ファイル:
  - `crates/bevy_app/src/world/map/boundary.rs`
  - `crates/hw_visual/src/material/terrain_surface_material.rs`
  - `assets/shaders/terrain_surface_material.wgsl`
  - `docs/world_layout.md`
- 完了条件:
  - [ ] LOD1 で far 側の見た目が破綻しない
  - [ ] LOD1 で境界リボンが残り、輪郭情報が維持される
  - [ ] LOD0 / LOD1 往復で明滅しない
- 検証:
  - `cargo check --workspace`
  - TopDown / 矢視でズーム往復
  - `--perf-scenario` で far ズーム時の FPS 傾向を比較

## M3: LOD2 導入（TopDown far overview）

- 変更内容:
  - far 用 `TerrainOverviewMap` と表示 entity を追加する。
  - TopDown のみ LOD2 で通常地形を隠し、overview へ切り替える。
  - 境界リボンは残し、必要なら far 用 simplification を適用する。
  - `TerrainChangedEvent` で `terrain_id_map` と overview image の両方を更新する。
- 変更ファイル:
  - `crates/bevy_app/src/world/map/spawn.rs`
  - `crates/bevy_app/src/world/map/terrain_metadata.rs`
  - `crates/bevy_app/src/systems/visual/terrain_material.rs`
  - `crates/bevy_app/src/plugins/startup/visual_handles.rs`
  - `docs/world_layout.md`
  - `docs/architecture.md`
- 完了条件:
  - [ ] TopDown far で地形 draw が少数化される
  - [ ] TopDown far で境界リボンが主輪郭として残る
  - [ ] 地形変更後も overview が stale にならない
  - [ ] 矢視では overview に入らない
- 検証:
  - `cargo check --workspace`
  - 岩除去など `TerrainChangedEvent` が出る操作後に far / near を往復
  - `--perf-scenario` で far ズーム時の FPS 傾向を比較

## M4: 再評価と chunk 化判断

- 変更内容:
  - M2 / M3 後の計測結果を見て、なお draw/extract cost が重い場合のみ chunk 化を起票する。
- 変更ファイル:
  - `docs/plans/...` または `docs/proposals/...`
- 完了条件:
  - [ ] chunk 化が不要か、次計画として必要かが明文化されている
- 検証:
  - 計測ログ比較

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `scale` 基準だけで閾値を決めて解像度差で壊れる | 中 | `tile_screen_px` を正本にする |
| LOD 切替時のポッピング | 中 | enter / exit を分けたヒステリシスを必須にする |
| overview が runtime 地形変更に追従しない | 高 | `TerrainChangedEvent` の consumer を増やし、更新経路を 1 箇所に集約する |
| 矢視で flat overview が破綻する | 高 | `LOD2` を TopDown 限定に固定する |
| shader 簡略化で near の見た目まで落ちる | 中 | LOD0 と LOD1 の分岐を明示し、デフォルトは LOD0 維持にする |
| overview と境界リボンの色・コントラストが競合する | 中 | far 用 overview のトーンと boundary material の強度をセットで調整する |
| chunk 化まで広げてスコープが肥大化する | 中 | M4 まで先送りし、M1〜M3 の成果を見てから判断する |

## 9. 検証計画

- 必須:
  - `cargo check --workspace`
- 手動確認シナリオ:
  - TopDown で zoom in / out を往復し、LOD0 / LOD1 / LOD2 の切替と復帰を確認
  - 矢視へ切り替え、LOD2 に入らないことを確認
  - 岩除去など terrain 変化後に far 表示が stale にならないことを確認
- パフォーマンス確認（必要時）:
  - `cargo run -p bevy_app -- --spawn-souls 500 --spawn-familiars 30 --perf-scenario`
  - 近景 / far 景で FPS 表示または profiler を比較

## 10. ロールバック方針

- どの単位で戻せるか:
  - `M1` 観測基盤
  - `M2` shader / boundary 切替
  - `M3` overview impostor
- 戻す時の手順:
  - M3 を戻しても M2 は残せる構成にする。
  - M2 を戻しても M1 の metric は残せる構成にする。
  - `WorldMap` や pathfinding には触れないため、ロールバックは visual 系だけで完結させる。

## 11. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
  - なし
- 未着手/進行中:
  - M1〜M4 未着手

### 次のAIが最初にやること

1. `tile_screen_px` をどう定義するかを `camera_sync.rs` と RtT viewport 前提で確定する。
2. LOD state Resource と hysteresis 付き閾値を先に入れる。
 3. M2 では boundary を消さず、地形本体だけ先に簡略化する。

### ブロッカー/注意点

- `docs/plans/world-map-lod-plan-2026-04-05.md` は作業木で削除されているため、この計画を正本とする。
- `LOD2` は TopDown 専用。矢視や section correctness を巻き込まない。
- まず削るべきは「全部を chunk にすること」ではなく、「far で地形本体の detail を描かないこと」。
- 境界リボンは遠景の主要情報なので、非表示前提では進めない。

### 参照必須ファイル

- `docs/world_layout.md`
- `docs/architecture.md`
- `crates/bevy_app/src/world/map/spawn.rs`
- `crates/bevy_app/src/world/map/boundary.rs`
- `crates/bevy_app/src/systems/visual/camera_sync.rs`
- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `assets/shaders/terrain_surface_material.wgsl`

### 最終確認ログ

- 最終 `cargo check`: `2026-04-06` / `not run (docs only)`
- 未解決エラー:
  - 未確認

### Definition of Done

- [ ] M1〜M3 が完了
- [ ] `docs/world_layout.md` / `docs/architecture.md` が同期済み
- [ ] TopDown far で地形コストが near より下がる
- [ ] `cargo check --workspace` が成功

## 12. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-06` | `Codex` | 初版作成。TopDown far overview impostor を主軸に、shader 簡略化と境界リボン無効化を組み合わせる方針へ整理 |

## 13. パフォーマンスレビューによる追記・改善案

アーキテクチャおよびパフォーマンスの観点から、本計画は「LOD2（Overview Impostor）の導入」「Screen-space metricの採用」「ヒステリシスの導入」「段階的アプローチ（チャンク化の先送り）」など、極めて堅実で効果的なアプローチをとっている。
実装にあたり、効果を最大化するために以下の懸念事項と改善案を追記する。

### 13.1 LOD2（Overview）更新時のCPU/GPU転送スパイク対策
- **懸念**: `TerrainChangedEvent` を受けてLOD2の遠景用画像を更新する際、100x100マップ全体のテクスチャを毎度再生成してGPUに丸ごと再転送（Upload）すると、地形変化（岩の採掘など）の瞬間にフレーム落ち（スパイク負荷）が発生するリスクがある。
- **改善案**: テクスチャ全体の再送ではなく、**変更があったタイルの周辺ピクセルのみを更新する部分更新**（Bevyの `Image` の更新機能やwgpuの `write_texture` 相当の機能を利用）の仕組みをM3の段階で確実に設計・実装すること。

### 13.2 「境界リボン」のLOD2におけるDraw Call残留対策
- **懸念**: LOD2で地形本体を1枚絵にしても、多数の境界リボン（ポリラインメッシュ）をそのまま描画し続けると、Draw Callがあまり減らず、CPU側の負荷削減効果が薄れる可能性がある。
- **改善案**: M3にある「far用 simplification（簡略化）」はオプションではなく**必須要件**として扱うべきである。
  - **最善策**: LOD2用のOverview Imageを生成する際、**境界線もそのテクスチャの中に焼き込んでしまい、境界リボンのEntity自体は遠景では非表示にする**。これによりDraw Callを劇的に削減できる。
  - **代替策**: 視認性などの観点からどうしても別Entityとして描画し続ける必要がある場合は、極限まで頂点数を減らしたLOD専用のリボンメッシュへ差し替えること。

### 13.3 LOD1のシェーダー分岐の仕組み
- **懸念**: LOD1（中景）でシェーダーを簡略化する際、WGSL内で単に `if (is_lod1)` のような動的分岐を使って重い処理をスキップしようとすると、GPUのアーキテクチャによっては両方の分岐を評価してしまい、期待ほど負荷が下がらないことがある。
- **改善案**: 可能な限り、Bevyのパイプラインレベルでの切り替え（異なるシェーダー定義やフラグを持つ別マテリアルへの差し替え、あるいは確実な静的分岐/Specialization Constant）を活用し、シェーダーのコンパイル段階で重い処理（macro overlay等）が完全に除外されるように構成すること。

### 13.4 タイル/ゾーンごとのバイオーム情報保持を前提とした対案の評価
- **前提**: `hw_world` の生成結果（`WorldMasks` の `grass_zone_mask`, `dirt_zone_mask` 等）をBevy側でもバイオーム情報として保持・活用する設計。
- **評価**:
  - 現在の計画（インポスター化）は、ピクセル単位のテクスチャで遠景を描画するため、バイオームの境界線やグラデーションをそのまま「1枚の画像」として焼き込むことができ、最も相性が良い。
  - **対案（Tilemap Shader / 1枚Quad化）の再評価**: もしタイル単位ではなく、より荒い「ゾーン単位」でのみバイオーム情報を持てば良いのであれば、1枚のQuadメッシュに対して、フラグメントシェーダーでSDF（Signed Distance Field）やバイオームマップをサンプリングして地形を描画するアプローチ（対案3）の実現性が高まる。しかし、依然として「近景での立体感（3Dメッシュの起伏）」とのシームレスな移行が課題となるため、現状では**「LOD2用のインポスターテクスチャ生成時にバイオーム色を反映させる」**という現行案の拡張として扱うのが最も安全かつパフォーマンスが高い。

### 13.5 将来的な「動的地形変更（Terrain Modification）」との相性評価
- **前提**: プレイヤーのアクション（採掘、整地、バイオーム変化など）により、ゲームプレイ中に地形の形状や種類が動的に変化するケース。
- **現行案（インポスター化）の評価**: **極めて良好**。
  - 現行案は既に `TerrainChangedEvent` を用いたインポスター用テクスチャの部分更新（ピクセル単位の書き換え）を想定しており、地形変更時の遠景更新コストが最小限に抑えられる設計です。
  - 近景（LOD0/1）においても、対象となる特定のタイル Entity のみを更新（メッシュやマテリアルの差し替え）すれば良いため、ECSの強み（個別Entityの独立性）をそのまま活かせます。
- **対案の評価**:
  - **対案1（GPU Instancing）**: 良好〜普通。地形変更時は、GPU上のインスタンスバッファやストレージバッファの該当インデックス部分を書き換える必要があります。実装難易度は上がりますが、バッファの部分更新ができれば高速です。
  - **対案2（チャンク化）**: **相性が悪い**。1つのタイルを変更するたびに、そのタイルが属するチャンク全体（例: 16x16タイル）の頂点メッシュを再生成（Re-mesh）し、GPUへ再転送する必要があります。これがスパイク負荷（フレーム落ち）の直接的な原因となり、非同期メッシュ生成などの複雑な仕組みが追加で必要になります。
  - **対案3（Tilemap Shader）**: テクスチャ変更のみであれば高速ですが、掘削など「3Dとしての形状変化（高さや立体感の追加）」を伴う場合、1枚のQuadに対するフラグメントシェーダー内でのディスプレイスメント/視差マッピング計算が極めて複雑になり、パフォーマンスとビジュアルの両面で破綻しやすいです。
- **結論**: 動的な地形変更を将来的に見据えた場合でも、特定のタイルだけを個別のEntityとして操作でき、遠景はテクスチャの部分書き換えで対応可能な**現行案が、最も柔軟性が高くパフォーマンスリスクが少ない（スパイクが起きにくい）アーキテクチャ**と言えます。
