# 構造・保守性・品質ゲート フォローアップ計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `structural-maintainability-followups-plan-2026-07-12` |
| ステータス | `Completed` |
| 作成日 | `2026-07-12` |
| 最終更新日 | `2026-07-15` |
| 作成者 | `Codex` |
| 親ロードマップ | [system-wide-correctness-refactoring-plan-2026-07-12.md](../system-wide-correctness-refactoring-plan-2026-07-12.md) |
| 前提 | runtime / Save-Load子計画完了。性能計画M0〜M7とM8判断は性能最適化の採否に必要であり、2026-07-15 の明示指示により、保存形式・grid update policy・hot pathを変えない M2 共通化には適用しない。 |
| 関連Issue/PR | `N/A` |

## 1. 目的

### 解決したい課題

- runtime計画M0後もproduction App compositionが`main.rs`に残り、simulation/presentation/root adapterの登録責務が見えにくい。
- SpatialGrid wrapperが`GridData`委譲と標準Transform updaterを重複実装している。
- component型をそのままgeneric tagにすると、`hw_logistics -> hw_spatial`の既存依存と循環し得る。
- Resource gridとGathering gridには特殊update policyがあり、一律generic化すると挙動退行する。
- `visual_test`に6件の`#[allow(clippy::too_many_arguments)]`が残り、`-D warnings`では検出できない。
- local toolchainと品質ゲートが未固定で、format baselineの差分有無を継続的に検出できない。

### 到達したい状態

- `main.rs`はplatform/backend/window/DefaultPlugins設定とgame plugin追加だけを持つ。
- production plugin compositionとsystem set設定はlibrary側に一意に置く。
- 標準Transform追跡gridのResource/SpatialGridOps/updater実装が共通化される。
- downstream domain型を`hw_spatial`から参照せず、独立tagとowner側wrapperで依存方向を維持する。
- Visibility/center等の特殊policyは専用systemとして残る。
- Clippy allowなしでall-target warnings 0を維持する。
- Rust toolchainをpinし、ローカルで再現できる品質ゲートを固定する。
- 全体rustfmtの差分が発生した場合は、機能変更と分離した単独コミットで完了する。

### 成功指標

- `main.rs`にgame module宣言、共有Resource定義、game system set構成が残らない。
- game plugin/systemの二重登録testが成功する。
- 標準SpatialGridの`SpatialGridOps`委譲実装が1箇所になる。
- `hw_spatial`のdependencyへ`hw_logistics`を追加しない。
- Resource/Gathering gridの現行filter・position sourceが維持される。
- `rg -n '#\[(allow|expect)\(clippy::' crates --glob '*.rs'`が0件、または恒久docsで個別承認された例外だけになる。本計画では0件を目標とする。
- ローカルのfmt、check、profiling compile、all-target clippy、testが成功する。

## 2. スコープ

### 対象（In Scope）

- `bevy_app` production App compositionとplugin責務整理。
- simulation/presentation/root-only adapterの登録境界文書化。
- `hw_spatial`標準gridのgeneric Resource/trait/updater。
- `hw_logistics`所有component用の具体sync wrapper。
- startup初期化、Save/Load reset、SpatialPlugin登録の追従。
- `visual_test` Clippy allow 6件のSystemParam等による構造修正。
- `rust-toolchain.toml`、local品質ゲート、全体format baseline。

### 非対象（Out of Scope）

- runtime挙動、task/event/obstacle/save formatの追加変更。
- 新crate追加。
- pathfinding/AI/transportアルゴリズム最適化。
- 全大ファイルの分割。
- UI/visualデザイン変更。
- GitHub Actions の実行結果・成功確認。既存 workflow は任意の補助とし、本計画の完了条件に含めない。

## 3. 設計判断

### 3.1 App composition

- runtime計画M0でmodule/shared Resourceはlibraryへ移動済みとする。
- `HellWorkersGamePlugin`がgame state、system sets、既存game pluginの追加を所有する。
- `main.rs`は`DefaultPlugins`のWindow/Log/Render設定後に`HellWorkersGamePlugin`を追加して`run()`する。
- simulation/presentationを無理に完全分離してfull headless Appを作ることは完了条件にしない。headless回帰はruntime計画の「対象system直接登録」を正本とする。
- ただし既存`LogicPlugin` / `DamnedSoulPlugin`内でroot-only visual adapterとsimulation登録が混在する箇所は分類し、意味のあるsub-pluginへ分ける。
- 同じsystem/observer/pluginが複数経路から登録されないことをApp build testで確認する。

### 3.2 SpatialIndex型

```rust
pub struct SpatialIndex<Tag> {
    data: GridData,
    marker: PhantomData<fn() -> Tag>,
}
```

- tagは`hw_spatial`所有のZST (`SoulIndexTag`, `StockpileIndexTag`等)とし、domain Component型を使わない。
- 既存のcustom cell sizeとgrid inspectionを失わないよう、`SpatialIndex<Tag>::new(GridData)`、`data`、`data_mut`、`into_data`を公開する。tuple field直参照ではなくこのAPIを使う。
- updaterはindex tagとtracked componentを分ける。

```rust
update_transform_spatial_index_system::<IndexTag, TrackedComponent>
```

- `Stockpile` / `TransportRequest` / `ResourceItem`の具体wrapperは所有者`hw_logistics`に残す。
- 標準generic updater対象はTransformを位置正本とし、Added<Component>/Changed<Transform>/RemovedComponents<Component>で同期するgridだけ。
- `ResourceSpatialGrid`はVisibility変更・Visibility removal再登録があるため専用updaterを維持する。
- `GatheringSpotSpatialGrid`は`GatheringSpot.center`を使い、timer由来Changedを無視するため専用updaterを維持する。
- system登録tupleの並列性を維持し、不必要な`.chain()`を追加しない。

### 3.3 品質baseline

- `rust-toolchain.toml`で`1.96.1`、`rustfmt`、`clippy`をpinする。実装開始時にBevy 0.19/workspaceがこのversionでbuild可能か再確認し、不可能なら同一PR内で最小互換versionへ調整する。
- `.github/workflows/ci.yml` は同じ command を実行する任意の補助とし、ローカル完了判定は外部CIに依存しない。
- `cargo fmt --all --check`が差分を報告した場合だけ、clean worktreeかつ並行sessionなしを確認して全体rustfmtをformat-only commitとして実施する。
- Clippy allowは警告抑制ではなくSystemParam/parameter object等で構造修正する。

## 4. 期待する影響

- production App compositionの一元化で、test起動と実ゲーム起動のplugin差分を減らす。runtime挙動の変更は意図しない。
- `SpatialIndex<Tag>`はzero-sized tagによる型分離を維持し、既存indexと同等のlookup計算量・メモリ特性を保つ。
- updater共通化で重複実装を減らす一方、Resource可視性とGathering中心座標の個別policyは維持する。
- toolchain/CI/format baselineは実行時性能を変えず、警告・整形差分・全target破損の早期検出を改善する。

## 5. マイルストーン

## M1: production App compositionのlibrary集約

### 変更内容

1. `HellWorkersGamePlugin`をlibraryへ追加する。
2. game state/resource/system set/既存pluginの登録をgame pluginへ移す。
3. `main.rs`をplatform/backend/window/DefaultPlugins + game plugin + runへ縮小する。
4. `LogicPlugin` / `DamnedSoulPlugin`の登録をsimulation、presentation adapter、root-only asset adapterに分類し、必要なsub-pluginへ分ける。
5. registration ownership表をarchitecture docsへ更新する。

### 主な変更ファイル

- `crates/bevy_app/src/{lib.rs,main.rs}`
- `crates/bevy_app/src/plugins/`
- `crates/bevy_app/src/entities/damned_soul/mod.rs`
- `crates/bevy_app/src/systems/soul_ai/mod.rs`
- `docs/architecture.md`
- `docs/cargo_workspace.md`
- `docs/crate-boundaries.md`

### 完了条件

- [x] `main.rs`がruntime shellだけになる
- [x] production起動時のplugin/resource/system setが移行前と同一
- [x] registration ownerが各system/observerにつき1箇所
- [x] runtime計画の最小test App helperを壊さない
- [x] crate boundary docs間に矛盾なし

### 検証

- App build/registration smoke test
- `cargo check -p bevy_app@0.1.0 --bin bevy_app`
- `cargo test -p bevy_app@0.1.0 --lib`
- `cargo check --workspace`

## M2: 標準SpatialIndexの共通化

### 変更内容

1. §3.2の`SpatialIndex<Tag>`、ZST tag、blanket `SpatialGridOps` implを追加する。
2. 標準Transform gridをtype aliasまたは薄いnewtypeへ置換する。
3. 標準updaterを`IndexTag`と`TrackedComponent`の2型引数で共通化する。
4. `hw_logistics::spatial_sync`のStockpile/TransportRequest wrapperを新APIへ接続する。
5. Resource/Gathering専用updaterはpolicyを維持し、Resource型だけ共通index storageを利用するかは重複削減量を見て決める。system policyは統合しない。
6. Startup/resource init、SpatialPlugin登録、Save/Load post-resetを新aliasへ追従させる。

### 主な変更ファイル

- `crates/hw_spatial/src/{grid.rs,lib.rs}`
- `crates/hw_spatial/src/{soul.rs,familiar.rs,designation.rs,blueprint.rs,floor_construction.rs,stockpile.rs,transport_request.rs}`
- `crates/hw_spatial/src/{resource.rs,gathering.rs}`（storage追従のみ、policy維持）
- `crates/hw_logistics/src/spatial_sync.rs`
- `crates/bevy_app/src/plugins/{startup/mod.rs,spatial.rs}`
- `crates/bevy_app/src/systems/save/{load.rs,reset.rs}`
- `docs/cargo_workspace.md`

### 完了条件

- [x] standard gridのGridData委譲とSpatialGridOps implが1箇所
- [x] `hw_spatial` dependencyに`hw_logistics`なし
- [x] Stockpile/TransportRequest tagがhw_spatial所有ZST
- [x] Resource visibility policy維持
- [x] Gathering center/Added-only policy維持
- [x] set所属・既存ordering constraint・並列性維持
- [x] load reset後に全indexが再構築される

### 回帰テスト

- tagごとのResource型分離/混線防止
- standard grid update/move/remove parity
- custom cell size / grid inspection API (`new` / `data` / `data_mut` / `into_data`)
- Resource Hidden/Visible/Visibility removal
- Gathering timer Changedで不要updateなし
- load reset後、pause中は再構築せずunpause後の最初のSpatial tickで再構築する schedule test

## M3: Clippy allowの構造的解消

### 変更内容

1. `visual_test`の6関数を関連parameterごとの`#[derive(SystemParam)]`へ集約する。
2. setup/input/building/soul/systemごとに責務境界を維持し、巨大な単一SystemParamを作らない。
3. `#[allow(clippy::too_many_arguments)]`を削除する。
4. 新しいClippy allow/expectを禁止する`rg` gateを追加する。

### 主な変更ファイル

- `crates/visual_test/src/{systems.rs,building.rs,soul.rs,setup.rs,input.rs}`
- `CLAUDE.md`または`docs/DEVELOPMENT.md`（gate記載）

### 完了条件

- [x] 対象6 allowが0件
- [x] workspace内Clippy allow/expectが0件
- [x] visual_test操作/描画挙動に変更なし
- [x] all-target Clippy warnings 0

### 検証

- `rg -n '#\[(allow|expect)\(clippy::' crates --glob '*.rs'`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo check -p visual_test`

## M4: toolchain・local品質ゲート・format baseline

### 変更内容

1. `rust-toolchain.toml`を追加し、Rust/rustfmt/clippy versionをpinする。
2. `cargo fmt --all --check`が差分を報告した場合だけ、clean worktree/並行sessionなしを確認して`cargo fmt --all`を単独commitで実行する。
3. local quality commandを`docs/DEVELOPMENT.md`へ記載する。
4. format後に全gateを実行する。

### 主な変更ファイル

- `rust-toolchain.toml`
- Rust source全体（format差分がある場合のformat-only commit）
- `docs/DEVELOPMENT.md`

### Local必須gate

1. `cargo fmt --all --check`
2. `cargo check --workspace`
3. `cargo check -p bevy_app@0.1.0 --lib --features profiling`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `! rg -n '#\[(allow|expect)\(clippy::' crates --glob '*.rs'`
6. `cargo test --workspace`

### 完了条件

- [x] pin toolchainでlocal全gate成功
- [x] local gateが新しい`#[allow(clippy::...)]` / `#[expect(clippy::...)]`を失敗として検出
- [x] format checkに意味上のコード変更なし
- [x] user/parallel session由来の差分をformat操作へ混入していない

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| App composition移動で二重登録 | system二重実行 | registration smoke test、owner表 |
| full headless分離まで広げる | scope肥大 | 対象system直接登録testを維持し、本計画の完了条件にしない |
| generic tagにdomain型使用 | crate循環 | hw_spatial所有ZSTを固定 |
| Resource/Gatheringを標準updaterへ統合 | visibility/center退行 | 専用policy維持と回帰test |
| type aliasでdiagnostic名が読みにくい | schedule debug低下 | 必要な具体wrapper/system名を残す |
| SystemParamが巨大化 | borrow conflict/可読性低下 | feature単位に複数paramへ分ける |
| global formatが並行差分を巻き込む | conflict/unrelated change | clean tree/parallel確認、format-only commit |
| toolchain pinが依存と非互換 | build失敗 | M4開始時にworkspace checkし最小互換versionへ調整 |

## 7. 検証計画

### 各マイルストーン必須

- 変更Rustファイルを個別rustfmt
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 対象crate test
- rust-analyzer workspace diagnostics 0件
- `git diff --check`

### 計画完了時

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo check -p bevy_app@0.1.0 --lib --features profiling`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `! rg -n '#\[(allow|expect)\(clippy::' crates --glob '*.rs'`
- `cargo test --workspace`
- `python scripts/update_docs_index.py`

## 8. ロールバック方針

- M1〜M4を独立コミットにする。
- M2はgrid family単位に分割可能だが、old/new Resourceを同時登録しない。
- M3はvisual_test feature単位に分割する。
- M4のtoolchain/local品質ゲート commitと、必要時のformat-only commitを分ける。
- archive時に必要なら`git add -f docs/plans/archive/<file>`を使う。

## 9. AI引継ぎメモ

### 現在地

- 進捗: M1〜M4完了。GitHub Actions の実行確認は本計画の対象外。
- 完了済み: M1 production App composition、M2 `SpatialIndex<Tag>` 共通化、M3 Clippy allow構造解消、M4 toolchain pin・local品質gate・format baseline。
- M2 は性能最適化の採否を前提にしない機械的な型所有・更新 policy 保持のリファクタリングとして、2026-07-15 の明示指示で先行した。性能計画の counter、algorithm、eager rebuild は変更していない。
- M4 の `cargo fmt --all --check` は差分なしで成功したため、dirty worktreeに対する全体formatは実行せず、format-only commitは生成していない。
- 通常の開始条件は runtime/Save-Load子計画、性能M0〜M7、条件付きM8の実施/skip決定済みとする。ただし `2026-07-15` の明示指示では、保存形式・grid update policy・hot pathを変えない M2 の型共通化も、この性能前提から独立した機械的リファクタリングとして先行した。
- M2後の local gate は `cargo fmt --all --check`、`cargo check --workspace`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` の順に成功した。
- `docs/proposals/hvac-plumbing-proposal.md`の既存変更は対象外。

### 次のAIが最初にやること

1. v0 support終了や新規 grid family を追加する場合は、M2の tag / policy 回帰testを更新する。

### ブロッカー/注意点

- full headless game pluginを新たな完了条件にしない。
- `hw_spatial -> hw_logistics`依存を追加しない。
- Resource/Gathering update policyをgeneric Transform updaterへ入れない。
- Clippy allowで警告を隠さない。
- global format前に並行sessionとdirty treeを必ず確認する。

### Definition of Done

- [x] M1〜M4完了
- [x] production plugin ownership一意
- [x] SpatialIndex共通化test成功
- [x] Clippy allow 0件
- [x] pinned toolchain/local全gate成功
- [x] docs/index更新、計画archive済み

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-12` | `Codex` | 全体計画の自己レビュー指摘を反映して新規作成 |
| `2026-07-12` | `Codex` | AI引継ぎの開始条件をmetadataどおり性能M0〜M7完了・M8判定後へ統一 |
| `2026-07-12` | `Codex` | 再レビューを反映し、package ID、Clippy allow CI gate、期待影響を明記 |
| `2026-07-15` | `Codex` | 明示指示により M1 の game plugin集約と M3 の Clippy allow構造解消を先行実施。M2/M4 の開始条件は維持。 |
| `2026-07-15` | `Codex` | M4 の Rust 1.96.1 pin、Linux native dependencyを含むCI、profiling compileを含むlocal品質gateを追加。format checkは差分なし、CI実行待ち。 |
| `2026-07-15` | `Codex` | 明示指示により M2 を先行実装。`SpatialIndex<Tag>` と crate 所有 ZST tagへ標準7 gridを集約し、Resource Visibility / Gathering center policyは専用 system のまま維持した。tag分離、add/move/remove、Visibility、Gathering Changed、SpatialPlugin / load reset再構築の回帰testを追加。 |
| `2026-07-15` | `Codex` | 実装後レビューを反映。custom `GridData` の構成・検査 API、物流wrapperの共通query alias、pause中のload reset非再構築、legacy shim TypePath固定と寿命保護非適用の回帰testを追加。 |
| `2026-07-15` | `Codex` | 指示により GitHub CI 実行確認を完了条件から除外し、ローカル品質ゲートだけで M4 を完了とした。 |
