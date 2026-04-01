# WFCマップ自動生成 実装計画（固定アンカー併用版）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-terrain-generation-plan-2026-04-01` |
| ステータス | `Draft` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-01` |
| 作成者 | `Codex` |
| 親マイルストーン | `docs/plans/3d-rtt/milestone-roadmap.md` **並行トラックB: WFC地形生成** |
| 関連提案 | `docs/proposals/3d-rtt/related/wfc-terrain-generation-plan-2026-03-12.md`（旧案。全面自動生成前提へ再整理） |
| 関連Issue/PR | `N/A` |

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

- **2段階生成にする。**
  1. 固定アンカーを確定する
     - `Site`
     - `Yard`
     - Yard 内固定の初期木材
     - Yard 内固定の猫車置き場
  2. そのアンカーを避けながら WFC で地形と資源帯を生成する

- **WFC は地形生成の中核として使う。**
  - `River`, `Sand`, `Dirt`, `Grass` の地形分布を WFC で決める。
  - River の大域構造は「横断する水系」という要件を保ちつつ、固定直線ではなく seed ごとに揺らげる余地を持たせる。
  - `Site/Yard` 内は `Grass` / `Dirt` のみ許可し、`River` / `Sand` 禁止にする。

- **木・岩は地形生成後に procedural 配置する。**
  - 森林帯候補や岩帯候補は WFC 後の地形・ゾーンマスクを使って決める。
  - `Site/Yard`、初期導線、猫車置き場、初期木材周辺には生成しない。
  - 既存の `TREE_POSITIONS` / `ROCK_POSITIONS` は最終的に廃止対象とする。

- **固定アンカーは絶対座標でなく、固定ゾーン基準で持つ。**
  - `Site/Yard` 自体は現行 `compute_site_yard_layout()` の固定配置を維持する。
  - 初期木材・猫車置き場は `Yard` の左上や中心からの相対オフセットで定義し、Yard 内固定を保証する。
  - 現在の `(58, 58)` は計画上の移行対象であり、そのまま維持しない。

- **公開契約は段階的に置き換える。**
  - `generate_base_terrain_tiles()` 単体では足りなくなるため、最終的には `GeneratedWorldLayout` のような pure 生成結果 struct を `hw_world` が返す形へ拡張する。
  - ただし移行途中では `generate_base_terrain_tiles()` wrapper を残し、段階的に startup 側を切り替える。

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

## 5. マイルストーン

## MS-WFC-1: 固定アンカー定義と生成結果モデル化

- 変更内容:
  - `Site/Yard` 固定領域、Yard 内固定の初期木材、Yard 内固定の猫車置き場を pure データとして定義する。
  - `GeneratedWorldLayout` のような、地形・木・岩・固定資源・固定施設アンカーをまとめた生成結果モデルを導入する。
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
  - [ ] 現在の絶対座標 parking を置換する方針がコード上で表現されている
- 検証:
  - `cargo check --workspace`

## MS-WFC-2: WFC 地形生成の中核実装

- 変更内容:
  - 固定アンカーを hard mask として WFC に渡し、`River/Sand/Dirt/Grass` を生成する。
  - `Site/Yard` 内は `Grass/Dirt` のみ許可する。
  - River はマップ横断性を維持しつつ seed に応じて形を変える。
  - 4方向・斜め方向・2x2 コーナーの禁止パターン validator を導入する。
- 変更ファイル:
  - `crates/hw_world/src/terrain.rs`
  - `crates/hw_world/src/mapgen.rs` または `crates/hw_world/src/mapgen/{solver,validate}.rs`
  - `crates/hw_world/src/river.rs`
- 完了条件:
  - [ ] `Site/Yard` 内に `River` / `Sand` が生成されない
  - [ ] 生成地形が validator を通る
  - [ ] 同一 seed で再現性がある
  - [ ] 別 seed で River / Dirt / Sand の分布が変化する
- 検証:
  - `cargo test -p hw_world`
  - `cargo check --workspace`
  - `cargo clippy --workspace`

## MS-WFC-3: 木・岩の procedural 配置

- 変更内容:
  - 地形生成結果を元に、木と岩の配置を自動生成する。
  - `Site/Yard` と Yard 内固定オブジェクト周辺を exclusion zone とする。
  - 既存の `TREE_POSITIONS` / `ROCK_POSITIONS` 依存を取り除く。
- 変更ファイル:
  - `crates/hw_world/src/layout.rs`
  - `crates/hw_world/src/mapgen.rs` または `crates/hw_world/src/mapgen/resources.rs`
  - `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`
- 完了条件:
  - [ ] 木・岩が固定座標テーブルなしで生成される
  - [ ] `Site/Yard` と固定 Yard オブジェクトに干渉しない
  - [ ] walkable / obstacle 条件が現行仕様と整合する
- 検証:
  - `cargo test -p hw_world`
  - `cargo check --workspace`
  - `cargo clippy --workspace`

## MS-WFC-4: Startup 統合と固定初期資源の Yard 内移行

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
- 検証:
  - `cargo check --workspace`
  - `cargo clippy --workspace`
  - `cargo run`

## MS-WFC-4.5: ドキュメントと検証整備

- 変更内容:
  - `docs/world_layout.md` を固定レイアウト仕様から「固定アンカー付き自動生成仕様」へ更新する。
  - `debug_assertions` と tests で生成 invariants を固定する。
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
- 検証:
  - `cargo test -p hw_world`
  - `cargo check --workspace`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| WFC が固定アンカーのせいで収束しない | 起動不能 | アンカー周囲は許可地形を広めに設定し、restart 上限と fallback を持つ。 |
| Site/Yard と River が近すぎて序盤導線が壊れる | gameplay 破綻 | Site/Yard 周辺に保護帯を設け、River 禁止距離を持たせる。 |
| 木・岩の procedural 配置で序盤が詰まる | pathfinding / logistics が停滞 | exclusion zone と最小通路幅を validator に含める。 |
| Yard 内固定物の座標が zone サイズ変更に弱い | 将来変更で破綻 | 絶対座標を廃止し、Yard 基準オフセットに統一する。 |
| 旧固定レイアウトの docs と実装が乖離する | 保守コスト増 | 実装完了と同時に `docs/world_layout.md` を更新する。 |

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
  - MS-WFC-1〜4.5 すべて未着手

### 次のAIが最初にやること

1. `Site/Yard` の固定領域と Yard 内固定アンカーを pure struct に落とす。
2. `GeneratedWorldLayout` のような生成結果モデルを先に定義する。
3. その後で WFC solver と procedural 木・岩配置に着手する。

### ブロッカー/注意点

- ユーザー条件:
  - `Site/Yard` は固定配置
  - `Site/Yard` 内は `Grass` または `Dirt` のみ
  - 猫車置き場は Yard 内固定
  - 初期木材は Yard 内固定
  - それ以外は自動生成
- 現在の `INITIAL_WHEELBARROW_PARKING_GRID = (58, 58)` は Yard 内固定条件と矛盾する。現状維持しないこと。
- `TREE_POSITIONS` / `ROCK_POSITIONS` / `INITIAL_WOOD_POSITIONS` をいきなり削除するのではなく、生成結果経由へ段階移行すること。

### 参照必須ファイル

- `crates/hw_world/src/mapgen.rs`
- `crates/hw_world/src/layout.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/mod.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/layout.rs`
- `crates/bevy_app/src/systems/logistics/initial_spawn/terrain_resources.rs`
- `docs/world_layout.md`
- `docs/plans/3d-rtt/milestone-roadmap.md`

### 最終確認ログ

- 最終 `cargo check`: `2026-04-01` / `not run`（計画更新のみ）
- 未解決エラー:
  - 未確認

### Definition of Done

- [ ] WFC が地形自動生成の中核として実装されている
- [ ] `Site/Yard` と Yard 内固定オブジェクトの制約が守られている
- [ ] 木・岩が procedural 配置に置き換わっている
- [ ] `cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace` が成功している

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-01` | `Codex` | ユーザー指定の固定アンカー条件を反映し、WFC を地形中心からマップ自動生成の中核へ再計画 |
