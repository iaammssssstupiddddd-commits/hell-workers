# WFCマップ自動生成 実装計画（固定アンカー併用版）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-terrain-generation-plan-2026-04-01` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-05` |
| 作成者 | `Codex` |
| 親マイルストーン | `docs/plans/3d-rtt/milestone-roadmap.md` **並行トラックB: WFC地形生成** |
| 関連提案 | `docs/proposals/3d-rtt/related/wfc-terrain-generation-plan-2026-03-12.md`（旧案。全面自動生成前提へ再整理） |
| 関連Issue/PR | `N/A` |

## サブ計画（マイルストーン別）

| マイルストーン | サブ計画ファイル | 概要 |
| --- | --- | --- |
| MS-WFC-0 | [`wfc-ms0-invariant-spec.md`](wfc-ms0-invariant-spec.md) | 生成 invariant・保護帯・validator 区分・golden seeds を文書として確定 |
| MS-WFC-1 | [`wfc-ms1-anchor-data-model.md`](wfc-ms1-anchor-data-model.md) | `AnchorLayout` / `GeneratedWorldLayout` / `WorldMasks` の型定義 |
| MS-WFC-2a | [`wfc-ms2a-crate-adapter-river-mask.md`](wfc-ms2a-crate-adapter-river-mask.md) | 外部 WFC crate 選定・アダプタ骨格・川マスク seed 付き帯生成 |
| MS-WFC-2b | [`wfc-ms2b-wfc-solver-constraints.md`](wfc-ms2b-wfc-solver-constraints.md) | WFC ソルバー統合・制約マスキング・deterministic retry/fallback |
| MS-WFC-2c | [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md) | lightweight validator（必須 4 チェック）+ debug validator 実装（**完了**・`mapgen/validate.rs`） |
| MS-WFC-3 | [`wfc-ms3-procedural-resources.md`](wfc-ms3-procedural-resources.md) | 木・岩の procedural 配置・ForestZone 生成・regrowth 接続準備 |
| MS-WFC-4 | [`wfc-ms4-startup-integration.md`](wfc-ms4-startup-integration.md) | bevy_app の startup を GeneratedWorldLayout に切り替え |
| MS-WFC-4.5 | [`wfc-ms45-docs-tests.md`](wfc-ms45-docs-tests.md) | docs 更新・golden seeds 回帰テスト・debug レポート整備 |

### 実装状況（2026-04 時点）

- **MS-WFC-0 / 1**: 仕様・`AnchorLayout` / `WorldMasks` / `GeneratedWorldLayout` 型は実装済み。
- **MS-WFC-2a**: 外部 `wfc` / `direction` 依存、`wfc_adapter` 骨格、保護帯 BFS、`seed` 付き `river_mask` / `fill_river_from_seed` 実装済み。
- **MS-WFC-2b**: `run_wfc`（`RunOwn::new_wrap_forbid` + `collapse`）、`post_process_tiles`、`generate_world_layout` の retry/fallback、スタブ地形 `generate_stub_terrain_tiles_from_masks` の削除済み。
- **MS-WFC-2c**: `mapgen/validate.rs` に `lightweight_validate` / `debug_validate` を実装済み。`generate_world_layout` の retry ループ内で lightweight 通過時のみ採用し、`ResourceSpawnCandidates` を埋める。詳細は [`wfc-ms2c-validator.md`](wfc-ms2c-validator.md)。
- **MS-WFC-3 以降**: 未着手。

## 1. 目的

- 解決したい課題:
  - 現状は [`crates/hw_world/src/mapgen.rs`](../../../crates/hw_world/src/mapgen.rs) の固定地形、[`crates/hw_world/src/layout.rs`](../../../crates/hw_world/src/layout.rs) の固定木・岩・初期木材座標、[`crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs`](../../../crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs) の固定 `Site/Yard` に依存しており、マップが毎回ほぼ同じになる。
  - 旧 WFC 案は地形だけを差し替える寄り方だったが、それでは「WFC を中核にした自動生成」の意味が弱い。
  - ただしユーザー指定の hard constraint として、`Site/Yard` は固定配置のまま残し、`Site/Yard` 内は `Grass` または `Dirt` のみ許可、初期木材と猫車置き場は Yard 内固定にする必要がある。
- 到達したい状態:
  - WFC を使って地形・森林帯・岩帯の基盤を自動生成し、固定アンカーである `Site/Yard`・初期木材・猫車置き場だけを保護したワールド生成に置き換える。
  - 固定アンカー外の木・岩・地形分布は seed に応じて変化し、毎回異なるマップになる。
  - 3D 地形表現側が期待している「孤立点ではない Dirt 領域」「禁止コーナーの抑制」を初期地形で満たす。
- 成功指標:
  - 同一 seed では同一マップ、別 seed では異なるマップが生成される。
  - `Site/Yard` は現行位置に固定され、その内側に `River` / `Sand` / 木 / 岩が生成されない。
  - 初期木材と猫車置き場は Yard 内の固定座標に配置される。
  - 木・岩は固定座標ではなく、生成地形に応じて配置される。
  - 少なくとも `Site↔Yard` が歩行連結であり、Yard から初期木材・猫車置き場・最低 1 つの水源/砂源/岩源へ到達可能である。
  - `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が通る。

## 2. スコープ

### 対象（In Scope）

- `hw_world` での地形自動生成
- `hw_world` での生成用アンカー／禁止領域データの定義
- `hw_world` での木・岩の配置計画生成
- `bevy_app` 初期スポーンが固定座標表ではなく生成結果を使うようにする変更
- `Site/Yard` 固定配置との整合
- Yard 内固定の初期木材・猫車置き場への変更
- WFC 生成結果の検証 helper / tests / debug assertions

### 非対象（Out of Scope）

- `Site/Yard` 自体の procedural 配置
- セーブデータへの seed 保存 UI
- runtime 中の地形変形や再生成
- RtT / `SectionMaterial` / 境界ブレンドのレンダリング処理
- 建物配置 AI や物流ルールの設計変更

## 3. 現状とギャップ

### 現状

- 地形:
  - [`crates/hw_world/src/mapgen.rs`](../../../crates/hw_world/src/mapgen.rs) は `River` 固定帯 + `Sand` 固定帯 + `(x+y)%30==0` の `Dirt` で構成される。
- 固定オブジェクト:
  - [`crates/hw_world/src/layout.rs`](../../../crates/hw_world/src/layout.rs) に `TREE_POSITIONS` / `ROCK_POSITIONS` / `INITIAL_WOOD_POSITIONS` がハードコードされている。
- 固定施設:
  - [`compute_site_yard_layout`](../../../crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs) は現行どおり中央固定の `Site` と右側固定の `Yard` を返す。
  - 初期猫車置き場は [`INITIAL_WHEELBARROW_PARKING_GRID`](../../../crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs) = `(58, 58)` に固定されているが、これは現在の `Site` 側にあり、今回の「Yard 内固定」と一致していない。
- 初期スポーン:
  - [`initial_resource_spawner`](../../../crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs) は、固定木 → 固定岩 → 固定木材 → 固定 `Site/Yard` → 固定猫車置き場の順でスポーンしている。

### 問題

- WFC を地形だけに留めると、木・岩・資源配置は依然として固定レイアウトのままで、自動生成の価値が限定的になる。
- `Site/Yard` を守らない生成にすると、序盤導線と物流が破綻する。
- 猫車置き場と初期木材の固定位置は、今後は「絶対座標」ではなく「固定 Yard アンカーからの相対位置」として管理すべきだが、現状はそうなっていない。

### 本計画で埋めるギャップ

- WFC を「地形だけの差し替え」ではなく、「固定アンカーを避けながら世界を埋める自動生成」の中核に引き上げる。
- `Site/Yard`・初期木材・猫車置き場だけを hard constraint とし、それ以外は seed ベースで変化する構成にする。
- 固定座標テーブル依存を減らし、生成結果データを startup が消費する流れへ改める。

## 4. 実装方針（高レベル）

### 方針

- **着手前に生成 invariant を先に固定する。**
  - アンカー領域
  - 保護帯
  - 必須資源到達条件
  - `golden seeds`
  を先に仕様化してから solver 実装に入る。

- **2段階生成にする。**
  1. 固定アンカーを確定する
     - `Site`
     - `Yard`
     - Yard 内固定の初期木材
     - Yard 内固定の猫車置き場
  2. そのアンカーを避けながら WFC で地形と資源帯を生成する

- **WFC は地形生成の中核として使う（§4.5 F1・F3・F4 参照）。**
  - **川**: タイル総数固定・幅 2〜4 の手続き生成でマスクを作り、WFC には **既に川として確定したセル**を渡す（川そのものを WFC だけで「伸ばす」前提にしない）。
  - **砂**: §4.5 F4（川隣接を主、それ以外は低頻度・目安 8 割が川隣接）。
  - **Grass / Dirt** 等の残りは外部 WFC crate 経由で、固定マスク（`Site/Yard`・川）を尊重する。
  - `Site/Yard` 内は `Grass` / `Dirt` のみ許可し、`River` / `Sand` 禁止にする。

- **木・岩は地形生成後に procedural 配置する。**
  - 森林帯候補や岩帯候補は WFC 後の地形・ゾーンマスクを使って決める。
  - `Site/Yard`、初期導線、猫車置き場、初期木材周辺には生成しない。
  - 木については初期配置だけでなく、**再生可能エリア（ForestZone 相当）** も同時に生成する。
  - 既存の `TREE_POSITIONS` / `ROCK_POSITIONS` は最終的に廃止対象とする。

- **固定アンカーは絶対座標でなく、固定ゾーン基準で持つ。**
  - `Site/Yard` 自体は現行 `compute_site_yard_layout()` の固定配置を維持する。
  - 初期木材・猫車置き場は `Yard` の左上や中心からの相対オフセットで定義し、Yard 内固定を保証する。
  - 現在の `(58, 58)` は計画上の移行対象であり、そのまま維持しない。

- **公開契約は段階的に置き換える。**
  - `generate_base_terrain_tiles()` 単体では足りなくなるため、最終的には `GeneratedWorldLayout` のような pure 生成結果 struct を `hw_world` が返す形へ拡張する。
  - ただし移行途中では `generate_base_terrain_tiles()` wrapper を残し、段階的に startup 側を切り替える。

- **生成結果は診断可能な形で残す。**
  - `GeneratedWorldLayout` には最終タイルだけでなく、`site_mask`、`yard_mask`、`river_mask`、`river_centerline`、要素別保護帯（debug report 上は合成 `protection_band` としても扱う）、`resource_spawn_candidates`、`forest_regrowth_zones` のような中間結果を保持する。
  - debug 時に「なぜその配置になったか」を追える形を優先する。

### 設計上の前提

- `Site/Yard` は固定配置のまま。
- `Site/Yard` 内の許可地形は `Grass` / `Dirt` のみ。
- 初期木材は Yard 内固定。
- 初期猫車置き場も Yard 内固定。
- それ以外の地形・木・岩は自動生成。

### crate 境界での方針

- `hw_world`
  - 生成アルゴリズム
  - 生成用マスク
  - 生成結果データ構造
  - validation
- `bevy_app`
  - 生成結果を消費して spawn する app shell
  - `Commands` / `GameAssets` 依存の具象スポーン

- **WFC ソルバー**
  - 実装は **外部 crate に依存**する（アルゴリズムの保守を crate 側に寄せる）。選定は MS-WFC-2 着手時に crates.io・ライセンス（MIT/Apache 等）・メンテ状況を確認し、`Cargo.toml` に明示する。**債務**としてバージョン追従・破壊的変更への対応を計画に含める。
  - crate 固有型は `hw_world` 全体へ漏らさず、`wfc_adapter` に閉じ込める。

### 4.5 確定方針（レビュー反映・2026-03-29）

以下は実装の前提として確定する。§4 の記述と矛盾する場合は **本節を優先**する。

| ID | 項目 | 方針 |
| --- | --- | --- |
| F1 | 格子 | **1 ゲームタイル = WFC の 1 セル**（`MAP_WIDTH`×`MAP_HEIGHT` フル解像度）。`WorldMap` との変換層を挟まない。 |
| F2 | 近傍と検証 | **初版はカーディナル（4 近傍）整合を WFC の主対象**とする。斜め・2×2 などの禁止パターンは **生成後の validator**（および必要なら局所修正・再試行）で扱う。**理由**: 多くの WFC 実装は辺の整合が素直で、対角まで同一ループに入れると制約結合が強く失敗しやすい。対角要件は後からルール拡張しやすい。 |
| F3 | 川 | **川タイル総数は seed から決まる固定値**（マップごとに一定）。**幅はセグメントごとに 2〜4 タイル**で、パスに沿って **連続的にランダム変化**させる（最小・最大は上記にクリップ）。形状は「横断水系」を満たす手続き生成（中心線＋幅）を先に確定し、**`river_centerline` と `river_mask` の両方**を `GeneratedWorldLayout` に保持したうえで、**川マスクを WFC の hard constraint に渡す**。 |
| F4 | 砂 | **川に辺接しない位置にも砂を出してよい**が、**出現頻度は川隣接より大幅に低く**する。目安として **全体の砂タイルのうちおおよそ 8 割は川に辺接**（4 近傍）を満たし、残りは許可セル（`Site/Yard` 外・他マスクと矛盾しない）に **低い重み**で散らす。実装は WFC のタイル重み＋後段の補正、または二段パスでよい。具体係数は定数化し、調整しやすくする。 |
| F5 | ソルバー | **外部 crate 依存**（§4 末尾）。`hw_world` 内は **アダプタ層**（マスク・`TerrainType`・seed の橋渡し）に留め、差し替え可能にする。 |
| F6 | 収束失敗 | **同一マスタ seed につき最大 N 回**（目安 64）まで再試行。再試行は **`master_seed + attempt_index` から導く deterministic な sub-seed 列**のみを使い、**master seed 自体は変更しない**。それでも失敗時は **同じ master seed から deterministic に得られる安全フォールバック**（未決定セルを `Grass` 等で埋める）へ落とす。これにより **同一 master seed → 同一最終マップ** を維持する。現行方針では debug/test でもフォールバックで続行し、検知は **`used_fallback` の warning・ログ・golden seed テスト**で担保する。正確な N は実装時に定数化。 |
| F7 | 資源保証 | 資源は **必須資源**（水源・砂源・岩源など序盤進行に必要）と **装飾/追加資源**に分ける。必須資源は hard constraint か validator 必達、追加資源は soft constraint として扱う。 |
| F8 | 保護帯 | `Site/Yard` の内側禁止だけでなく、外周にも **保護帯**を設ける。少なくとも River 禁止、岩禁止、高密度木禁止を含め、序盤導線を保護する。具体幅は定数化する。 |
| F9 | 到達可能性 invariant | validator は 2 段構成にする。**軽量 validator** は `Site↔Yard`、Yard から初期木材・猫車置き場・最低 1 つの水源/砂源/岩源への到達可能性を確認する。**重い debug validator** は距離や通路幅など追加診断を行う。到達判定は `hw_world::pathfinding` と同じ walkable 契約を使う。 |
| F9.5 | 木の再生エリア | 木の procedural 初期配置と **再生可能エリア** は別々に持たず、同じ生成フェーズで決める。`forest_regrowth_zones` を pure データとして生成し、`regrowth` は固定座標群ではなくこの zone 定義を参照する。初期木はその zone 内の一部として spawn する。 |
| F10 | パフォーマンス | 初期実装は正しさ優先。**後から最適化しやすいよう**、`river_mask` / `wfc_adapter` / `validate` / `GeneratedWorldLayout` を **モジュール分割**する。想定する最適化の置き場だけ先に書く（下記「F10 補足」）。 |

**F10 補足（最適化の舵を切りやすくする予定地）**

- **縮小ドメイン**: 固定マスク（川・`Site/Yard`）適用後の「未確定セルだけ」に WFC をかける。
- **階層化**: 粗いブロックで大まかに決めてから細グリッドへ（インターフェースは `GeneratedWorldLayout` 不変）。
- **並列**: 独立したチャンクに分割できるよう、生成を純粋関数＋明示的入力に閉じる。
- **計測**: `startup` 生成時間を dev でログ（閾値超過で警告）。ボトルネックが判明したら上記のどれかへ切り替え。

### 4.6 当初案との差分（メモ）

- 当初レビューでは「初版は自前 WFC」を提案したが、**F5 により外部 crate 採用に変更**した。

### 4.7 デバッグ・回帰運用

- **golden seeds**
  - 少数の固定 seed 集を持ち、毎回その seed 群で validator を通す。
  - 少なくとも「標準」「川が曲がりやすい」「保護帯ぎりぎり」の代表 seed を含める。
- **生成レポート**
  - dev 専用で、地形・アンカー・保護帯・資源候補・最終配置を色分けしたダンプを出せるようにする。
  - 形式は PNG、テキスト、あるいはデバッグ overlay のいずれでもよいが、pure 生成結果から再現可能にする。

## 5. マイルストーン

### MS-WFC-0: 生成 invariant 仕様化

- 変更内容:
  - 固定アンカー、保護帯、必須資源、到達可能性、`golden seeds` を先に仕様化する。
  - debug レポートに何を出すかもここで決める。
- 変更ファイル:
  - `docs/plans/3d-rtt/wfc-terrain-generation-plan-2026-04-01.md`
  - `docs/world_layout.md`（先行で仕様メモを入れる場合）
- 完了条件:
  - [ ] 必須資源と追加資源の区分が定義されている
  - [ ] 保護帯の対象と幅が定義されている
  - [ ] lightweight / debug validator の責務が分かれている
  - [ ] `golden seeds` の運用方針が定義されている
- 検証:
  - 文書レビュー

### MS-WFC-1: 固定アンカー定義と生成結果モデル化

- 変更内容:
  - `Site/Yard` 固定領域、Yard 内固定の初期木材、Yard 内固定の猫車置き場を pure データとして定義する。
  - `GeneratedWorldLayout` のような、地形・木・岩・固定資源・固定施設アンカーをまとめた生成結果モデルを導入する。
  - `site_mask`、`yard_mask`、`river_mask`、`river_centerline`、要素別保護帯（および合成 `protection_band`）、`resource_spawn_candidates`、`forest_regrowth_zones` などの診断用中間結果も保持できる形にする。
  - 現行の固定 `(58, 58)` 駐車位置を廃止し、Yard 基準オフセットへ置き換える方針を確定する。
- 変更ファイル:
  - `crates/hw_world/src/layout.rs`
  - `crates/hw_world/src/mapgen.rs` または `crates/hw_world/src/mapgen/types.rs`
  - `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`
  - `docs/plans/3d-rtt/wfc-terrain-generation-plan-2026-04-01.md`
- 完了条件:
  - [ ] `Site/Yard` 固定領域を pure に取得できる
  - [ ] Yard 内固定の木材・猫車置き場アンカーを pure に取得できる
  - [ ] 生成結果モデルが定義されている
  - [ ] 診断用中間結果の保持場所が定義されている
  - [ ] 木の再生可能エリアを保持する pure データの置き場が定義されている
  - [ ] 現在の絶対座標 parking を置換する方針がコード上で表現されている
- 検証:
  - `cargo check --workspace`

### MS-WFC-2: WFC 地形生成の中核実装

- 変更内容:
  - **外部 WFC crate** を `Cargo.toml` に追加し、`hw_world` にアダプタ層を実装する（§4.5 F5）。
  - **川マスク**を先に生成（総タイル数固定・幅 2〜4・§4.5 F3）。`crates/hw_world/src/river.rs` は固定タイル列挙から **seed 付き帯生成**へ段階移行。
  - 固定アンカーを hard mask として WFC に渡し、残りを `Sand` / `Dirt` / `Grass` 等で埋める（砂の扱いは §4.5 F4）。
  - `Site/Yard` 内は `Grass/Dirt` のみ許可する。
  - **4 近傍ベースの WFC**＋生成後 **validator**（斜め・2×2 等、§4.5 F2）。
  - lightweight validator に **到達可能性**（§4.5 F9）を含める。
- 変更ファイル:
  - `Cargo.toml`（workspace / `hw_world`）
  - `crates/hw_world/src/terrain.rs`（`TerrainType` は現状維持。ルール追加は別モジュール推奨）
  - `crates/hw_world/src/mapgen.rs` および `crates/hw_world/src/mapgen/{wfc_adapter,validate}.rs` 等
  - `crates/hw_world/src/river.rs`
- 完了条件:
  - [ ] `Site/Yard` 内に `River` / `Sand` が生成されない
  - [ ] 生成地形が validator を通る
  - [ ] 必須資源が最低保証される
  - [ ] 同一 seed で再現性がある
  - [ ] 別 seed で River / Dirt / Sand の分布が変化する
  - [ ] 同一 seed で fallback 経路に入っても最終マップが再現する
- 検証:
  - `cargo test -p hw_world`
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### MS-WFC-3: 木・岩の procedural 配置

- 変更内容:
  - 地形生成結果を元に、木と岩の配置を自動生成する。
  - 木については **初期配置** と **再生可能エリア** を同時に生成し、`regrowth` が参照する zone 定義へ接続する。
  - `Site/Yard` と Yard 内固定オブジェクト周辺を exclusion zone とする。
  - 必須資源到達を阻害しないよう、保護帯と通路幅を維持する。
  - 既存の `TREE_POSITIONS` / `ROCK_POSITIONS` 依存を取り除く。
- 変更ファイル:
  - `crates/hw_world/src/layout.rs`
  - `crates/hw_world/src/mapgen.rs` または `crates/hw_world/src/mapgen/resources.rs`
  - `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`
  - `crates/bevy_app/src/world/regrowth.rs`
- 完了条件:
  - [ ] 木・岩が固定座標テーブルなしで生成される
  - [ ] 木の再生可能エリアが pure データとして生成される
  - [ ] 初期木配置が再生可能エリアと矛盾しない
  - [ ] `regrowth` が固定配置前提ではなく生成された zone を参照する
  - [ ] `Site/Yard` と固定 Yard オブジェクトに干渉しない
  - [ ] walkable / obstacle 条件が現行仕様と整合する
  - [ ] `Site↔Yard` と Yard から最低 1 つの水源/砂源/岩源への到達可能性を壊さない
- 検証:
  - `cargo test -p hw_world`
  - `cargo check --workspace`
  - `cargo clippy --workspace`

### MS-WFC-4: Startup 統合と固定初期資源の Yard 内移行

- 変更内容:
  - `initial_resource_spawner` が `GeneratedWorldLayout` を使って spawn するように変更する。
  - 初期木材を Yard 内固定アンカーから spawn する。
  - 初期猫車置き場を Yard 内固定アンカーから spawn する。
  - 旧固定座標依存を削除する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`
  - `crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs`
  - `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`
  - `crates/bevy_app/src/systems/logistics/initial_spawn/facilities.rs`
- 完了条件:
  - [ ] initial spawn が生成結果を使う
  - [ ] 初期木材が Yard 内固定位置に置かれる
  - [ ] 猫車置き場が Yard 内固定位置に置かれる
  - [ ] 旧 parking 絶対座標への依存が消えている
  - [ ] 初期 spawn 後も `Site↔Yard` と Yard から固定/必須資源への到達可能性が維持される
- 検証:
  - `cargo check --workspace`
  - `cargo clippy --workspace`
  - `cargo run`

### MS-WFC-4.5: ドキュメントと検証整備

- 変更内容:
  - `docs/world_layout.md` を固定レイアウト仕様から「固定アンカー付き自動生成仕様」へ更新する。
  - `debug_assertions` と tests で生成 invariants を固定する。
  - `golden seeds` を使った回帰確認と生成レポート出力の運用を文書化する。
  - 木の再生エリアが初期木配置と別管理ではなく、生成結果の一部として扱われることを文書化する。
  - RtT 側の WFC 参照が新しい前提と矛盾しないよう整理する。
- 変更ファイル:
  - `docs/world_layout.md`
  - `docs/plans/3d-rtt/milestone-roadmap.md`
  - `docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md`
  - `crates/hw_world/src/mapgen.rs` または `validate.rs`
- 完了条件:
  - [ ] world_layout が自動生成前提に更新されている
  - [ ] `Site/Yard` / Yard 内固定オブジェクト / 自動生成対象の境界が docs に明記されている
  - [ ] tests / debug assertions で主要 invariants を検証している
  - [ ] `golden seeds` と生成レポートの運用が文書化されている
- 検証:
  - `cargo test -p hw_world`
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| WFC が固定アンカーのせいで収束しない | 起動不能 | アンカー周囲は許可地形を広めに設定し、restart 上限と fallback を持つ。 |
| Site/Yard と River が近すぎて序盤導線が壊れる | gameplay 破綻 | Site/Yard 周辺に保護帯を設け、River 禁止距離を持たせる。 |
| 木・岩の procedural 配置で序盤が詰まる | pathfinding / logistics が停滞 | exclusion zone と最小通路幅を validator に含める。 |
| 見た目は正しいが序盤必須資源へ到達できない | gameplay 破綻 | validator に `Site↔Yard` と水源/砂源/岩源への到達可能性を追加する。 |
| Yard 内固定物の座標が zone サイズ変更に弱い | 将来変更で破綻 | 絶対座標を廃止し、Yard 基準オフセットに統一する。 |
| 旧固定レイアウトの docs と実装が乖離する | 保守コスト増 | 実装完了と同時に `docs/world_layout.md` を更新する。 |
| 外部 WFC crate の破壊的更新・非メンテ | ビルド失敗・セキュリティ | 依存を最小にし、パッチバージョンの追従方針を `cargo_workspace.md` に一行残す。 |

## 7. 検証計画

- 必須:
  - `cargo test -p hw_world`
  - `cargo check --workspace`
  - `cargo clippy --workspace`
- 手動確認シナリオ:
  - `cargo run` で複数回起動し、地形・木・岩の分布が毎回変わることを確認する。
  - `Site/Yard` が毎回同じ位置にあり、内部が `Grass` / `Dirt` のみであることを確認する。
  - Yard 内に初期木材と猫車置き場が固定位置で生成されることを確認する。
  - Site/Yard 周辺に木・岩・River が食い込んでいないことを確認する。
  - `Site` から `Yard`、および `Yard` から初期木材・猫車置き場・最低 1 つの水源/砂源/岩源へ到達できることを確認する。
- パフォーマンス確認（必要時）:
  - startup の生成時間をログ化し、許容範囲を超える場合は solver の mutable 領域を見直す。

## 8. ロールバック方針

- どの単位で戻せるか:
  - `GeneratedWorldLayout` 導入前後で段階的に戻せるようにする。
  - startup 側は旧固定 spawn 経路を一時的に残せるように段階移行する。
- 戻す時の手順:
  1. startup 側を旧固定 spawn 経路へ戻す。
  2. `mapgen` の内部を legacy generator に戻す。
  3. procedural 木・岩配置を固定座標表へ戻す。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 完了済みマイルストーン:
  - なし
- 未着手/進行中:
  - MS-WFC-0〜4.5 すべて未着手

### 次のAIが最初にやること

1. MS-WFC-0 として、保護帯・必須資源・到達可能性・`golden seeds` を先に固定する。
2. `Site/Yard` の固定領域と Yard 内固定アンカーを pure struct に落とす。
3. `GeneratedWorldLayout` のような生成結果モデルを先に定義する。
4. **外部 WFC crate を選定**（ライセンス・API・`no_std` 要件なし）し、`hw_world` にアダプタの骨組みを追加する。

### ブロッカー/注意点

- ユーザー条件:
  - `Site/Yard` は固定配置
  - `Site/Yard` 内は `Grass` または `Dirt` のみ
  - 猫車置き場は Yard 内固定
  - 初期木材は Yard 内固定
  - それ以外は自動生成
- 現在の `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` は Yard 内固定条件と矛盾する。現状維持しないこと。
- `TREE_POSITIONS` / `ROCK_POSITIONS` / `INITIAL_WOOD_POSITIONS` をいきなり削除するのではなく、生成結果経由へ段階移行すること。
- seed 再現性を壊さないため、**master seed は変更しない**。再試行は deterministic な sub-seed 列で行うこと。

### 参照必須ファイル

- `crates/hw_world/src/mapgen.rs`
- `crates/hw_world/src/layout.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`
- `docs/world_layout.md`
- `docs/plans/3d-rtt/milestone-roadmap.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-04-02` / `not run`（計画更新のみ）
- 未解決エラー:
  - 未確認

### Definition of Done

- [ ] WFC が地形自動生成の中核として実装されている
- [ ] `Site/Yard` と Yard 内固定オブジェクトの制約が守られている
- [ ] 木・岩が procedural 配置に置き換わっている
- [ ] 木の再生可能エリアが生成結果と整合している
- [ ] `golden seeds` と生成レポートを含む回帰運用が成立している
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Codex` | ユーザー指定の固定アンカー条件を反映し、WFC を地形中心からマップ自動生成の中核へ再計画 |
| `2026-04-02` | `Codex` | 生成後 validator に到達可能性 invariant を追加。seed 再現性を壊さない deterministic retry / fallback 方針へ修正。 |
| `2026-04-02` | `Codex` | 外部 WFC crate 前提の補足に合わせて MS-WFC-2 を明確化。更新日・最終確認ログを同期。 |
| `2026-04-02` | `Codex` | `golden seeds`、生成レポート、必須/追加資源、保護帯、2段 validator、`MS-WFC-0` を追加。 |
| `2026-04-02` | `Codex` | 木の再生可能エリア（`forest_regrowth_zones`）を生成結果へ統合し、MS-WFC-3 / docs / DoD に反映。 |
