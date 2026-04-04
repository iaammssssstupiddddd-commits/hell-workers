# MS-WFC-4.5: ドキュメントと検証整備

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `wfc-ms45-docs-tests` |
| ステータス | `docs 一次掃除済み・tests/golden/debug 拡張は継続` |
| 作成日 | `2026-04-01` |
| 最終更新日 | `2026-04-05` |
| 親計画 | [`wfc-terrain-generation-plan-2026-04-01.md`](wfc-terrain-generation-plan-2026-04-01.md) |
| 前MS | [`wfc-ms4-startup-integration.md`](wfc-ms4-startup-integration.md) |
| 前提 | MS-WFC-4 の startup 本接続は実装済み。`generate_world_layout()` は本番経路で使われ、`GeneratedWorldLayoutResource` を通じて地形・初期木/岩・Yard 内固定物・regrowth 初期化が同じ layout を共有している |

---

## 1. この文書の位置づけ

MS-WFC-4.5 は、WFC 実装そのものではなく **周辺の説明・検証・運用を現状実装へ揃える**段階である。  
現時点では docs と tests は部分的に進んでいるため、本書は
「新規導入計画」ではなく **残件整理と収束条件の明文化**として扱う。

---

## 2. すでに達成されていること

### 2.1 docs 側

`docs/world_layout.md` はすでに大きく更新されており、少なくとも以下は反映済みである。

- `generate_world_layout(master_seed)` が本番相当の生成経路であること
- `AnchorLayout::aligned_to_worldgen_seed` を使うこと
- `forest_regrowth_zones` / `rock_field_mask` / `terrain zone` の存在
- startup が `GeneratedWorldLayout` を地形描画・初期木/岩・初期木材・猫車置き場・regrowth 初期化で共有すること

つまり、MS-WFC-4.5 の docs 作業は **ゼロからの全面更新ではない**。
主な仕事は **残っている stale 記述の除去**である。

**2026-04-04 追記**: `docs/debug-features.md`・`docs/world_layout.md`（暫定表現の整理）・`crates/bevy_app/src/world/README.md`・`milestone-roadmap.md`（トラック B）・親計画 `wfc-terrain-generation-plan-2026-04-01.md`（実装状況・§3 現状）を本番経路に合わせて更新済み。

### 2.2 tests 側

`hw_world` にはすでに WFC 関連テストが複数入っている。

- `crates/hw_world/src/mapgen.rs`
  - 同一 seed の deterministic 性
  - 別 seed で地形が変わること
  - `river_mask` / `final_sand_mask` と最終 terrain の整合
  - `Site/Yard` 内に `River/Sand` が入らないこと
- `crates/hw_world/src/mapgen/validate.rs`
  - `GOLDEN_SEED_STANDARD = 10182272928891625829` で `lightweight_validate()` が通ること
  - `water_tiles` / `sand_tiles` が埋まること
  - 意図的に壊した layout が validate で落ちること
- `crates/hw_world/src/mapgen/resources.rs`
  - 木・岩の exclusion 条件
  - forest zone / rock field との整合
  - `validate_post_resource()` が通ること
  - resource layout の deterministic 性
  - fallback resource layout が空でないこと

したがって、MS-WFC-4.5 の test 作業は
**新規に `golden_seeds.rs` を作ることが必須**なのではなく、
**既存テスト群をどう curated な回帰セットに整理するか**が中心になる。

### 2.3 debug 側

追加の `debug_report.rs` は未実装だが、診断が全くないわけではない。

- `generate_world_layout()` は `debug` / テストビルドで `debug_validate()` の warning を `eprintln!` する
- validate 失敗時は retry 中に `[WFC validate] ...` が出る
- startup ログには `seed`, `attempt`, `fallback` が出る

現状は **軽量ログ診断は存在するが、明示的な report 出力機構は未整備**という状態である。

**2026-04-04 追記**: 上記のうち seed / ログの意味づけは `docs/debug-features.md` に本番経路として記載済み。

---

## 3. 現在のギャップ

### 3.1 stale な docs 記述（一次対応済み 2026-04-04）

以下は **2026-04-04 に修正を入れた**。残る用語として `preview_river_min_y` の「プレビュー」は **関数がアンカー無しで川マスクを試算する**意味であり、暫定ゲーム経路とは別。

| ファイル | 状態 |
| --- | --- |
| `docs/debug-features.md` | 本番 startup 経路・ログ例に更新済み |
| `docs/world_layout.md` | `spawn.rs` の暫定表現を除去し、seed 共有経路を明記 |
| `crates/bevy_app/src/world/README.md` | `spawn.rs` を `GeneratedWorldLayout` 本接続に更新 |
| `docs/plans/3d-rtt/milestone-roadmap.md` | トラック B の MS-WFC-3/4/4.5 を現状に合わせて更新 |
| `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` | 実装状況・§3 現状を更新 |

### 3.2 tests の不足

現状のテストは有用だが、MS-WFC-0 で想定していた **curated な golden seed 運用**としては未完成である。

不足しているのは主に次の点。

- `STANDARD=10182272928891625829` 以外の named seed が定着していない
- `WINDING_RIVER` / `TIGHT_BAND` / `RETRY` に相当する seed がコード上で未固定
- seed の責務が `mapgen.rs` / `validate.rs` / `resources.rs` に分散している
- CI/運用上の「この seed 群を見れば regression が分かる」という入口がまだない

### 3.3 debug report の不足

以下はまだ未実装である。

- `HELL_WORKERS_DEBUG_WORLDGEN=1` や `--debug-worldgen` のような明示トリガー
- ASCII マップダンプ
- PNG レポート出力
- `target/debug_reports/` への保存運用

現時点では `debug_validate()` の stderr 出力が唯一の組み込み診断であり、
**見た目の比較や seed アーカイブ用途には弱い**。

### 3.4 受入状態の整理不足

`docs/plans/3d-rtt/ms-3-6-terrain-surface-plan-2026-03-31.md` は
`A/D 実装済み（WFC 後の S0・B は保留）` のままであり、これは直ちに誤りではない。  
ただし WFC 側が進んだ以上、

- S0 スクリーンショットを今の生成結果で撮るか
- B（隣接ブレンド）を本当に継続判断するか

は **再評価して記録する段階**に来ている。

---

## 4. MS-WFC-4.5 の実際の作業範囲

### A. docs の整合

優先度順:

1. `docs/debug-features.md` の「WFC 川プレビュー seed」を本経路前提の説明へ修正
2. `docs/world_layout.md` の暫定/preview 文言を除去
3. `crates/bevy_app/src/world/README.md` の `spawn.rs` 説明を修正
4. `docs/plans/3d-rtt/milestone-roadmap.md` の WFC トラック進捗を現実に合わせる
5. `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` の実装状況を現実に合わせる

（1)〜5) は 2026-04-04 に反映済み。以降は §3.2 以降の整理。）

### B. tests の整理

この MS でやるべきことは「テストゼロから新規追加」ではなく、
既存テストを **golden seed 運用として再編するか判断すること**である。

候補:

- `STANDARD=10182272928891625829` を正式な `GOLDEN_SEED_STANDARD` として共通化する
- `WINDING_RIVER` / `TIGHT_BAND` / `RETRY` に該当する代表 seed を探索して固定する
- 既存の deterministic / validate / resource-path テストをその seed 群で回す
- 入口を 1 か所にまとめる
  - 例: `validate.rs` か `mapgen.rs` に `golden seed smoke` 群を集約
  - ただし専用 `golden_seeds.rs` は、実際に共有が増えるまでは必須ではない

### C. debug 運用の判断

debug report については 2 段階で考える。

- 最低ライン:
  - 現在の `debug_validate()` / startup ログを docs に正しく書く
- 拡張ライン:
  - ASCII dump / PNG dump / opt-in flag を別実装として追加する

この MS の完了条件としては、**少なくとも最低ラインは必須**。
拡張ラインは実装コスト次第で同 MS か後続小タスクへ分離してよい。

---

## 5. 変更ファイルと責務

| ファイル | この MS での責務 |
| --- | --- |
| `docs/debug-features.md` | preview-only 記述を除去し、現行 startup 経路の seed / log / debug warning を説明 |
| `docs/world_layout.md` | 暫定接続扱いの文言を除去し、現行 startup 共有経路に統一 |
| `crates/bevy_app/src/world/README.md` | `spawn.rs` の説明を暫定プレビューから本接続へ修正 |
| `docs/plans/3d-rtt/milestone-roadmap.md` | 並行トラック B の進捗更新 |
| `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` | 実装状況の stale 記述を修正 |
| `crates/hw_world/src/mapgen.rs` / `validate.rs` / `resources.rs` | 既存テストを golden seed 運用として整理する場合の受け皿 |
| `crates/hw_world/src/mapgen/debug_report.rs` | 追加する場合のみ。ASCII/PNG report の置き場候補 |

---

## 6. 完了条件チェックリスト

### docs

- [x] `docs/debug-features.md` が preview-only ではなく現行 startup 経路を説明している
- [x] `docs/world_layout.md` の暫定表現（`spawn.rs` 経路・seed）が整理されている（`preview_river_min_y` は API 名として残す）
- [x] `crates/bevy_app/src/world/README.md` が現行実装を説明している
- [x] `docs/plans/3d-rtt/milestone-roadmap.md` の WFC トラック進捗が現実と一致している
- [x] `docs/plans/3d-rtt/archived/wfc-terrain-generation-plan-2026-04-01.md` の実装状況が現実と一致している

### tests

- [ ] `STANDARD=10182272928891625829` を含む代表 seed 群の扱いが docs またはコードで明文化されている
- [ ] `cargo test -p hw_world` 上で、地形 deterministic / validate / resource-path の回帰入口が把握しやすくなっている
- [ ] 代表 seed 群のうち少なくとも 1 つは fallback なしの正常系、必要なら retry 系もカバーしている

### debug

- [x] 少なくとも現行の `debug_validate()` / startup ログの使い方が docs に記録されている
- [ ] 追加 report を実装する場合は、トリガーと出力先が明記されている

### 検証

- [x] `cargo test -p hw_world`
- [x] `cargo check --workspace`
- [x] `cargo clippy --workspace -- -D warnings`

---

## 7. 現時点の判断

MS-WFC-4.5 のうち、**主要ドキュメントの stale 除去（§3.1 一覧）**は 2026-04-04 に一段落した。  
未完了なのは、主に

- curated golden seed 運用の明文化・テスト入口の集約（§3.2）
- explicit な debug report 機構の有無（§3.3）
- 地形表面計画との受入再評価（§3.4）

である。

そのため、この MS は「巨大な未着手タスク」ではなく、
**実装済み WFC を説明可能・検証可能な形へ仕上げる収束タスク**として扱うのが正しい。

---

## 8. 推奨順序

1. docs の stale 記述を先に消す
2. 既存テスト群から代表 seed 運用を整理する
3. 追加 report を入れるか、現行ログ運用で閉じるかを決める
4. roadmap と親計画のステータスを最後に更新する

---

## 9. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-05` | `Codex` | 現行実装に合わせた計画整理に加え、`cargo test -p hw_world` / `cargo check --workspace` / `cargo clippy --workspace -- -D warnings` の通過を再確認。`golden_seeds.rs` 新設前提を撤回し、既存テスト整理中心の方針へ修正 |
| `2026-04-04` | — | `debug-features` / `world_layout` / `bevy_app` world README / `milestone-roadmap` / 親計画を本番経路に同期。§3.1 を一次完了扱いに更新。検証に `clippy -D warnings` を明記 |
| `2026-04-01` | `Copilot` | wfc-terrain-generation-plan-2026-04-01.md から分割・詳細化 |
