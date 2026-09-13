# ライブラリ・開発ツールの導入／置換評価

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `library-tooling-evaluation-proposal-2026-09-13` |
| ステータス | Review |
| 作成日 / 最終更新日 | 2026-09-13 |
| 作成者 | Codex |
| 調査対象 | primary `master`、`eec38b3e6371f847da8c2a5dac7848ea6fc3b3e6` |
| 関連計画 | [優先開発ツール5項目の完了計画](../plans/archive/priority-development-tools-plan-2026-09-13.md)（D01/D03/D05/D06/T01。その他は未計画） |
| 関連Issue/PR | N/A |

## 1. 背景と問題

現行の実装・依存・検証・アセット制作を横断し、ライブラリやツールで減らせる不具合、作業時間、保守負担を評価する。
調査時点の公式資料とリポジトリを照合した提案であり、候補のインストール・依存更新・移行・性能比較は実施していない。

後続の実装ではD01/D03/D05/D06/T01の5項目を導入し、GitHub上の受入も完了した。導入・監査・受入結果は
[完了計画](../plans/archive/priority-development-tools-plan-2026-09-13.md)と[開発ガイド](../DEVELOPMENT.md#dependency-update-prとci)を参照する。以下の棚卸しは導入前の調査記録である。

**推奨は、依存監査・更新管理、Python/CIの静的検査、不変条件テストを先に補うこと。**
実行時の変更では、配布に向けた保存先の整備と設定ファイルの書き込み堅牢化が有力である。
Bevy、独自のAI・物流・セーブ・入力、native検証基盤を全面置換する根拠は見つからなかった。

## 2. 目的と評価方法

評価対象は、依存保守、ビルド、テスト、Python、CI、開発環境、保存、入力/UI、AI/空間探索、描画、アセット、配布の65項目。
全世界の製品の列挙ではなく、各責務について「追加」「部分置換」「既存活用」「維持」を比較する。

| 判断 | 意味 |
| --- | --- |
| P1 | 次の小規模作業として優先する。既存導入の有無を確認する項目や、限定PoCを含む |
| P2 | 具体的な機能要求・測定結果を条件に試す |
| 保留 | 現時点では移行費用・重複・互換性の問題が効果を上回る |
| 維持 | 既存実装または既存導入を活用する |

負担の「小」は局所設定・単一境界、「中」はdriverやテスト・複数箇所の統合、「大」は保存形式・実行順序・描画経路の移行、
「極大」はエンジン規模の再実装を意味する。工数や高速化率の実測値ではない。
優先度は現在の課題、置換で失う契約、導入後の保守までを基にした設計判断である。

## 3. 現状の棚卸し

| 領域 | 確認した事実 | 根拠 |
| --- | --- | --- |
| 規模 | workspace 13 crates、Rust source 967 files、`scripts/` Python 58 files。lockfileは559 package entries（workspace・対象外platformを含み、実行バイナリの依存数ではない） | root / `crates/*/Cargo.toml`、`Cargo.lock`、source inventory |
| Rust / Engine | Rust 1.96.1を固定。Bevy 0.19.0、wgpu 29.0.4、RON 0.12.0、Serde 1.0.228、WFC 0.10.7 | `rust-toolchain.toml`、`Cargo.lock` |
| 乱数 | 直接依存はrand 0.8、lockには0.8.5と0.10.2が共存。WFCも0.8.5に依存 | manifest、`dev.py cargo -- tree --locked --offline --workspace -i rand@0.8.5 --depth 2` |
| ビルド | mold、dynamic linking、依存opt-level 2、incremental、lane、RAM/永続storage/activity guardを導入済み | `.cargo/config.toml`、`Cargo.toml`、`scripts/dev.py`、`cargo_runtime.py` |
| 品質 | unittest、独自契約検査、fmt、workspace check/test、Clippy `-D warnings`、profiling各feature検査を一本化 | `scripts/dev.py::verify`、`.github/workflows/ci.yml` |
| 依存・CI設定 | rootにdeny / nextest / Ruff / Dependabot / Renovateの設定は見つからない。CI ActionはSHA固定、Cargo cacheあり | root設定・`.github/`。GitHub側のアラート設定は未確認 |
| ローカルツール | PATHでmold、bacon、cargo-expandを確認。deny/audit/nextest/llvm-cov/machete/sccache/Ruff/uv等は未検出 | 読み取りのPATH調査。別venvや別端末への未導入を意味しない |
| Python | driverは標準ライブラリのみ。Blender外部vendorにはvenvのpytest/Ruff/mypy手順が既存 | `scripts/dev.py`、`tools/blender_ai_workflow/README.md` |
| UI | Bevy 0.19 `EditableText` / `SelectAllOnFocus`、BSN・標準widgetを利用済み | `crates/hw_ui/src/widgets/text_field.rs`、settings UI |
| 保存 | RON allow-list、staging検証、Entity remap、rehydrate、rollback、atomic saveを実装。保存rootを注入可能 | [save_load.md](../save_load.md)、`crates/bevy_app/src/systems/save/` |
| 設定 | 独自RON形式・StorageRoot・検証・適用契約。書き込みは`std::fs::write` | [settings.md](../settings.md)、`systems/settings/persistence.rs` |
| 計測 | 決定的scenario、CSV、native allocator、Tracy、RenderDoc、Capture→Memoryの逐次検証が既存 | [performance-profiling.md](../performance-profiling.md) |
| アセット | Blender、hardened MCP、Khronos＋Wall/Door独自検査、承認hash・promotion、Syncthing原本分離が既存 | [blender-setup.md](../blender-setup.md)、[assets_workflow.md](../assets_workflow.md) |

アセットのファイルサイズ集計はPNG 154個＝107,567,271 B、TTF 13個＝28,180,336 B、GLB 37個＝1,053,388 B。
これはディスク上の在庫であり、ロード対象・展開後サイズ・VRAM・圧縮可能量の測定ではない。
最初のサイズ調査は画像・フォントに向け、幾何圧縮だけを優先しない。

## 4. 優先する導入候補

### 4.1 依存・CI・開発作業

| ID | 候補と判断 | 適用先と期待する効果 | 負担・条件・公式資料 |
| --- | --- | --- | --- |
| D01 | **cargo-deny: P1** | lock/依存graphのadvisory・license・source・禁止依存検査を`dev.py verify`へ追加。docsでは推奨済みだが定常gateにない | 小〜中。実依存に合う方針を作り、重複versionを一律拒否しない。未保守と脆弱性を区別。DB更新を伴う。[checks](https://embarkstudios.github.io/cargo-deny/checks/index.html) |
| D02 | cargo-audit: 保留、D01の簡易代案 | 脆弱性検査だけ先行するなら小さく始められる | 小。cargo-deny採用時に同じ監査を毎回二重実行する必要は薄い。今回の調査は脆弱性監査の実行結果ではない。[RustSec](https://github.com/rustsec/rustsec) |
| D03 | **Dependabot: P1** / Renovate: P2 | CargoとGitHub Actions更新を少数のPRにまとめる。Bevy/render群は関連更新として扱う | 小。週次等の限定運用、自動mergeなし。Bevy 0.x minorはAPI移行。独自tool revisionまで統合する必要が出たらRenovateを比較。両方を同時導入しない。[Dependabot](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-options-reference) |
| D04 | **cargo-machete: P1** / cargo-udeps: P2 | 未使用の直接依存を探す。既存crate境界検査とは別の穴を埋める | 小。macroやrenameの誤検知を確認してから削除。`--with-metadata`はlock変更の可能性があり初回は避ける。精度不足が出た場合だけudepsのnightly/build負担を比較。[machete](https://github.com/bnjbvr/cargo-machete) |
| D05 | **Ruffのroot tooling拡張: P1** | `scripts/`の未定義名・未使用import等を追加検査。Blender外部venvでの既存利用から対象を広げる | 小〜中。まず限定ルールで運用し、一括formatとは分ける。doctorのstdlib起動を維持。tool version固定。[Ruff](https://docs.astral.sh/ruff/) |
| D06 | **actionlint: P1** / zizmor: P2 | Actionsの構文・式・型検査を追加。workflow権限や入力処理を広げる時にzizmorの監査も有用 | 小。現状のSHA固定・read権限・verify入口を継承。既存CIに問題が発見されたとの主張ではない。[actionlint](https://github.com/rhysd/actionlint)、[zizmor](https://docs.zizmor.sh/) |
| D07 | Gitleaks: P1 | 既存repo hygieneのファイル名・pattern検査を、秘密文字列の検査で補う | 小〜中。まず差分検査と履歴の初回確認を分け、fixtureの誤検知を限定除外。CLIとAction提供形態の条件を導入時に確認。[公式](https://github.com/gitleaks/gitleaks) |
| D08 | bacon設定の明文化: P1、任意 | インストール済みbaconから`dev.py check`等へ接続し、変更後の確認を簡単にする | 小。既定のraw Cargo jobを使わず、laneとbusyを尊重。監視対象から巨大生成物を除く。[設定](https://dystroy.org/bacon/config/) |
| D09 | uv: P2、tooling限定 | Pillow等を使うasset scriptsやRuffのversion・環境を再現可能にする | 中。script lockかtooling専用projectを選ぶ。stdlibの`dev.py`起動をuv必須にしない。Blender同梱Python/vendor環境は別管理。[script管理](https://docs.astral.sh/uv/guides/scripts/) |
| D10 | lychee: P2 | 既存`check_docs.py`をfragment・外部URL検査で補完 | 小〜中。local検査とネットワーク検査を分け、archive除外を継承。索引生成・stale path検査は維持。[CLI](https://lychee.cli.rs/guides/cli/) |
| D11 | `dev.py`→just / xtask: 保留 | 入口と保護契約は現在のdriverに集約されている | 大。実装言語変更自体で解消する課題がない。追加ツールをdriverの内側へ統合する。ローカル根拠: `scripts/dev.py`、[validation workflow](../development-infra/validation-storage-workflow.md) |
| D12 | rust-analyzer / docsrs MCP、bacon、cargo-expand: 維持 | 共有backendとidle解放、Bevy版を固定した一次API確認を継続 | 小。別IDEや追加MCPへの切替だけでは解析の多重常駐は解消しない。[共有運用](../development-infra/rust-analyzer-mcp.md) |

### 4.2 テスト・ビルド・保守性

| ID | 候補と判断 | 適用先と期待する効果 | 負担・条件・公式資料 |
| --- | --- | --- | --- |
| T01 | **proptest: P1、局所導入** | 予約・建築・解体・座標変換・pathfinding・照明pure coreの不変条件を生成入力で検査 | 中。意味のある性質とgeneratorを設計し、失敗を縮小して固定回帰例へ。既存事例テストを維持。[公式](https://github.com/proptest-rs/proptest) |
| T02 | nextest: P2 | テスト選別・timeout・process分離・並列制限・JUnitをRust test工程へ追加 | 中。まずdriver guard対応。doctest非対応のため`cargo test --doc`を残す。test並列数は`CARGO_BUILD_JOBS`とは別管理。速度向上は未測定。[公式](https://nexte.st/)、[test groups](https://nexte.st/docs/configuration/test-groups/) |
| T03 | cargo-llvm-cov: P2 | pure coreとsave検証の未試験箇所を発見する | 中〜大。LLVM工具・instrumented build・専用出力が増える。既定の`llvm-cov-target`とclean処理を管理し、通常/review cacheを対象にしない。全workspace率の足切りを初手にしない。[公式](https://github.com/taiki-e/cargo-llvm-cov) |
| T04 | insta: P2 | 正規化したsave schema・catalog等の構造化出力を差分レビュー | 小〜中。Entity ID・時刻・map順を正規化。既存Help exact snapshotと人による意味レビューを置換・自動承認しない。[公式](https://insta.rs/docs/) |
| T05 | cargo-fuzz: P2 | セーブheader/body、asset manifest等の入力境界でpanic・過大入力・拒否経路を探す | 中〜大。まず既存回帰test/proptest。nightly、sanitizer、corpus/出力管理が必要。対象OS制約があり全platform gateにはしない。[公式](https://github.com/rust-fuzz/cargo-fuzz) |
| T06 | Miri: P2、限定 / cargo-geiger: 保留 | Miriは純Rustのunsafe周辺の動的検査候補。unsafe使用量の一覧は必要時の調査補助 | 中。GPU/FFIを含むゲーム全体の受入を代替できない。unsafe件数を安全性の合否にしない。[Miri](https://github.com/rust-lang/miri)、[geiger](https://github.com/geiger-rs/cargo-geiger) |
| T07 | cargo-hack: P2 | 既存profiling feature検査で漏れる組合せを限定して検査 | 中〜大。powerset総当たりは避ける。`--no-dev-deps`はmanifestを一時編集するためshared/frozen treeでは使わない。guard統合を先行。[公式](https://github.com/taiki-e/cargo-hack) |
| T08 | sccache: ローカル保留、CIはP2 | CIの非incremental依存compile再利用を現cacheと比較 | 中。incremental crateやlink呼出はcache制約あり。feedbackの`CARGO_INCREMENTAL=1`を無効にして導入しない。hit率・時間・追加diskを測定。[Rust制約](https://github.com/mozilla/sccache/blob/main/docs/Rust.md) |
| T09 | Swatinem/rust-cache: P2、CI限定 | 現在の`actions/cache`とkey管理・保存対象・restore時間を比較 | 中。既存cacheがあるため新導入による改善は未確認。workspace crate除外やcleanupの挙動を確認し、長寿命review環境へ流用しない。[公式](https://github.com/Swatinem/rust-cache) |
| T10 | mold / Rust固定 / Cargo `--locked`: 維持 | すでに導入済み。別linker・toolchainへの変更は測定または修正要求を起点にする | 小。既存最適化を「未導入」として重ねない。[mold](https://github.com/rui314/mold)、[rustup](https://rust-lang.github.io/rustup/overrides.html#the-toolchain-file) |
| T11 | Bevy 0.19.1のpatch更新: P2、単独評価 | lockは0.19.0。公式0.19.1 releaseを確認したため、修正一覧と現行不具合の関連を精査する価値がある | 中〜大。patchでも保存・renderer・入力の回帰確認が必要。関連Bevy crateを揃え、wgpu直参照との整合を確認。一般ツール導入と混ぜない。[release](https://github.com/bevyengine/bevy/releases/tag/v0.19.1) |

### 新Cargoサブコマンドの導入を止めている具体的な前提

`scripts/dev.py::run_command`のcompilation判定は`build/check/clippy/test/run/...`という列挙である。
調査時点では`nextest`、`llvm-cov`、`hack`等が含まれず、`dev.py cargo -- ...`で環境を正規化するだけでは
activity lease、`require_mutable`、RAM guardを取得しない。**外側をdriverで包むだけで全保護が効くという前提は誤り。**

これらを試す前に、subcommandのcompile・manifest変更・clean・独自outputの挙動を分類し、
`build_coordination.py` / `cargo_runtime.py` / `validation_storage.py`へ適合させる。
成功条件は、busy/frozen/低RAM時に子processが開始されないこと、lane継承、出力先の固定、
終了したjobの整理とreview cache保全である。現在のguardを回避する実行例は本提案に載せない。

## 5. 実行時ライブラリの導入／置換

以下は製品の保存・入力・ゲーム意味に触れるため、導入時にはHelp impactと対応する仕様書を同時に確認する。
外部crateの「Bevy 0.19対応」は公式対応表・manifestの確認であり、このworkspaceでのcompile/動作合格を意味しない。

| ID | 候補と判断 | 適用先と期待する効果 | 負担・条件・公式資料 |
| --- | --- | --- | --- |
| R01 | **directories 6 / Bevy標準dirs: P1、配布前** | `SaveStorageRoot`の`saves/`と`SettingsStorageRoot`の`settings/`というCWD依存を解消。settingsだけなら既存`bevy::platform::dirs::preferences_dir()`で足りる | 中。data/config/cacheを区別するなら`ProjectDirs`。旧path移行・portable mode・fixture root注入・書込不可を設計。[ProjectDirs](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html)、[Bevy 0.19 dirs](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_platform/src/dirs/mod.rs) |
| R02 | **設定保存の既存atomic I/O再利用: P1** | settingsの`std::fs::write`を、`systems/save/atomic_file.rs`の汎用部分に寄せる。新規crateなしで不足を埋める | 中。save専用header/slot処理とは分離。temp・commit・sync・失敗時旧file保持を検証。atomicwrites/confyへの全面移行は優先しない。根拠: 両persistence実装と[save仕様](../save_load.md) |
| R03 | Bevy App Settings / confyへの全面移行: 保留 | 標準化による保守削減の余地はあるが、現在のroot注入・RON検証・適用順序を失う | 中〜大。Bevy 0.19公式はTOML、Plugin build時ロード、app_name由来の保存先で、現StorageRoot契約と異なる。既存saveのdurability契約を自動的に満たすものでもない。[公式説明](https://bevy.org/news/bevy-0-19/#app-settings)、[0.19実装](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_settings/src/lib.rs) |
| R04 | fluent-bundle / bevy_fluent 0.15: P2、製品要件次第 | 英語literalのUIと日本語Helpを翻訳key・fallback・数値/複数形処理で統一する | 中〜大。bevy_fluent 0.15はBevy 0.19対応表を確認。settings/notificationsから開始。Help stable ID・exact承認・font fallback・runtime text分類を同時設計。[Fluent](https://github.com/projectfluent/fluent-rs)、[Bevy対応表](https://github.com/kgv/bevy_fluent) |
| R05 | leafwing-input-manager 0.21: P2 | rebind/gamepadが必要な場合に`input_actions/`の物理入力→action部分を比較 | 大。Bevy 0.19対応。ただし0.21で従来UI吸収featureが削除。exact chord・overlay・focus・in-progress drag・競合解決は残す。[公式](https://github.com/Leafwing-Studios/leafwing-input-manager)、[変更点](https://github.com/Leafwing-Studios/leafwing-input-manager/blob/main/RELEASES.md) |
| R06 | bevy_enhanced_input 0.26: P2、R05との二者比較 | context/observerによるaction処理を既存`UiIntent`の前段と比較する | 大。Bevy 0.19対応。pending capture・modifier一致・save禁止gesture・cancel順を先に確認。入力ライブラリを二つ同時導入しない。[公式](https://github.com/simgine/bevy_enhanced_input) |
| R07 | EditableText / 標準widgets / BSN: 維持 | `hw_ui/src/widgets/text_field.rs`等で利用済み。新規text-input/UI frameworkより既存利用を広げる | 小〜中。Bevy 0.19のcode内BSN利用と、将来の`.bsn` asset loaderを混同しない。0.19で後者は未提供。[0.19公式](https://bevy.org/news/bevy-0-19/) |
| R08 | Bevy Feathers: P2、開発パネル限定 | dev用の数値入力・list等が不足した時の候補 | 中。製品の`UiTheme`と既存画面を全面交換する効果は未確認。必要widgetだけ比較。[公式widget](https://bevy.org/news/bevy-0-19/#more-feathers-widgets) |
| R09 | 標準bevy_audio 0.19: P2、音の追加時は第一候補 | AudioPlayer/AudioSourceの既存使用は見つからず、rootのaudio featureも未指定。UI音・完了通知・環境loopから要件確認 | 中。音数上限・重複抑制・pause/load reset・音量設定を設計する新機能である。[audio](https://docs.rs/bevy_audio/0.19.0/bevy_audio/)、[0.19 features](https://docs.rs/crate/bevy/0.19.0/features) |
| R10 | bevy_kira_audio 0.26: P2、標準audioで不足時 | 複数channel・tween等のmixing制御が必要なら比較 | 中。Bevy 0.19対応表を確認。公式はBevy audio featureとの併用を避けるよう記載。基本的なSEだけならR09を優先。[公式](https://github.com/niklasei/bevy_kira_audio) |
| R11 | **worldgenの名前付きPRNG・生成version: P1、依存更新前** | `hw_world/src/mapgen/`のStdRng利用と同seed同layout契約を明確にする。現rand 0.8.5に対応するChaCha12Rng等を比較 | 中〜大。StdRngは将来のアルゴリズム変更を許す。生成器だけ固定しても分布・gen_range・WFC更新による結果変化は防げない。worldgen versionと固定seed corpusを先に設計。rand重複削減だけを目的に一括更新しない。[0.8.5 source](https://docs.rs/crate/rand/0.8.5/source/src/rngs/std.rs)、[生成契約](../map_generation.md) |
| R12 | bevy_rand 0.15.2: P2 | runtimeの分散したthread_rngを、replay・再現可能なAI検証が必要な場合に整理 | 中〜大。Bevy 0.19対応。安定seed・entityごとのstream・state保存・reset・system実行順を設計。global RNG一つでは順序依存が残る。[公式](https://docs.rs/crate/bevy_rand/0.15.2) |
| R13 | pathfinding 4.16: P2、oracle/比較backend | 現A*に対する到達性・costの比較用、または局所backendの保守削減候補 | 中〜大。現`pathfinding/core.rs`はscratch再利用、8方向、corner禁止、door cost、stable tie-break、budgetのDeferredを持つ。汎用A*への交換で同じ契約や高速化は保証されない。[公式](https://github.com/evenfurther/pathfinding) |
| R14 | rstar: P2、局所benchmark | 不均一分布・広い矩形・大きいfootprintの問い合わせが律速なら比較 | 中。現`hw_spatial/src/grid.rs`の差分更新・typed index・owner filterを維持。検索だけでなく移動actorの更新費用とallocationも測る。[公式](https://github.com/georust/rstar) |
| R15 | vleue_navigator 0.16 / Polyanya: 保留 | 自由形地形やany-angle経路を製品仕様にする場合の候補 | 大。Bevy 0.19対応は確認済み。ただし現gridのdoor cost・建築変更・budgetをnavmeshへ翻訳する層が必要。[公式](https://github.com/vleue/vleue_navigator) |
| R16 | big-brain等の汎用Utility AI: 保留 | 既存のPerceive/Decide/Execute、10Hz/wake-up、Top-K候補処理を維持 | 大。domain scorer/予約/中断は移行後も必要。確認した公開big-brain 0.22はBevy 0.15依存で、0.19対応は未確認。Codeberg移転告知があり、開発停止と断定しない。[公開crate](https://docs.rs/crate/big-brain/0.22.0)、[移転告知](https://github.com/zkat/big-brain) |
| R17 | wfc 0.10.7: 維持 | WFCはすでに導入済み。`mapgen/wfc_adapter.rs`の制約・後処理、retry/fallback、到達性検証を保つ | solver不具合を再現した場合だけ局所修正/交換を評価。別solverは地形・seedの再受入が必要。[採用元](https://github.com/gridbugs/wfc) |
| R18 | bevy_world_serialization維持 / bevy_save交換: 保留 | 標準serializationをすでに使い、domain preflight・rollback・rehydrate・WorldEpochを独自に保証 | 大。一般snapshot機能では代替できない。確認したbevy_save公開2.0.1+4はBevy 0.16.1依存で0.19対応未確認。[標準](https://docs.rs/crate/bevy_world_serialization/0.19.0)、[bevy_save](https://docs.rs/crate/bevy_save/2.0.1+4) |
| R19 | postcard / rmp-serde等のbinary保存: 保留、計測後PoC | serialize時間・bodyサイズが問題なら、RONの代替bodyを比較 | 大。Serde対応はDynamicWorld/Reflect互換やschema migrationを保証しない。外部header・旧loader・catalog・schema版を保持。RONとbinaryの往復だけでdomain復元を合格にしない。[Postcard](https://docs.rs/crate/postcard/1.1.3)、[serialization](https://docs.rs/crate/bevy_world_serialization/0.19.1) |
| R20 | lz4_flex等のsave body圧縮: P2 | `SaveTransactionMetrics`でI/Oやサイズが問題と分かった場合、RONを保つ代案 | 中〜大。平文headerの後ろだけ圧縮し、解凍上限・破損・CPU・peak allocationを比較。binary化と一度に変えない。[LZ4](https://docs.rs/crate/lz4_flex/0.14.0) |
| R21 | thiserror: P2、境界の局所整理 | persistence/asset境界等の手書きError実装を簡潔にできるか調べる | 小〜中。既存typed failureとplayer-safe通知を維持。String/enumを全域置換する理由はない。[公式derive](https://docs.rs/thiserror/latest/thiserror/derive.Error.html) |
| R22 | SmallVec等の小容量container: P2、割当測定後 | 短い候補・近傍・予約op配列がallocation hotspotなら候補 | 中。inline容量でcomponentやstackが膨らむこともある。既存scratch/pool再利用と比較し、Vec全域の置換はしない。[SmallVec](https://docs.rs/crate/smallvec/latest) |
| R23 | petgraph: P2、新しい設備networkの要件次第 | HVAC等で汎用graph操作が複雑になった際の候補 | 中〜大。現grid/Room/予約を単にgraphへ変える効果は未確認。trunkは開発中構成のため採用releaseを指定する。[公式](https://github.com/petgraph/petgraph) |

## 6. 描画・計測・アセット

| ID | 候補と判断 | 適用先と期待する効果 | 負担・条件・公式資料 |
| --- | --- | --- | --- |
| A01 | **標準GPU診断＋既存Tracy活用: P1** | system/frame CSVだけで原因が分からない時にGPU queueやpass診断を使う。Bevy 0.19は`trace_tracy`時にRenderDiagnosticsPluginを登録 | 小〜中。現CSVのframe timeをGPU timeと解釈しない。adapterのtimestamp能力を確認し、diagnostic runを正式baselineと区別。[0.19 profiling](https://github.com/bevyengine/bevy/blob/v0.19.0/docs/profiling.md#tracy-renderqueue) |
| A02 | heaptrack: P1、割当原因調査時 | 現allocatorの総量・scope集計から、割当元backtraceへ掘り下げる | 小〜中。symbol、短時間採取、保存先を管理。Memory会計を置換せず、instrumented実行時間を性能baselineにしない。[公式](https://github.com/KDE/heaptrack) |
| A03 | samply / perf: P2 | Tracyのsystemより細かいCPU関数・native library内のhotspotを調べる | 小。Linuxのperf権限とunwind品質を確認し、原因不明のCPU負荷に限定。[samply](https://github.com/mstange/samply) |
| A04 | **oxipng: P1、staging数枚のPoC** | 約102.6 MiBのPNG在庫の配布・同期サイズをlossless圧縮で減らせるか調べる。runtime依存は増えない | 小。decoded RGBAと色管理metadataを維持。透明RGBを変える`--alpha`は使用しない。既存承認hashを一括更新しない。VRAM削減とは別。[公式](https://github.com/oxipng/oxipng) |
| A05 | glTF Transform `inspect`: P2 | 新規importのgeometry/texture/material統計を補助表示 | 小。Node/Sharpの保守が増える。既存Khronos・Wall/Door専用validatorを維持。[CLI](https://gltf-transform.dev/cli) |
| A06 | KTX-Software / KTX2: P2 | 実測でtexture帯域・VRAMが問題なら、外部textureから限定試行 | 中。feature・transcoder・adapter format・alpha・色空間・zoom画質を確認。KTX2直接読込とglTF内`KHR_texture_basisu`対応は別。[Khronos](https://github.com/KhronosGroup/KTX-Software)、[Bevy 0.19対応表](https://docs.rs/bevy_gltf/0.19.0/bevy_gltf/#supported-khr-extensions) |
| A07 | gltfpack / meshoptimizer / Draco一括適用: 保留 | 現GLB在庫は約1 MiBで、描画問題には透明sortによるbatch分断等がある。圧縮はその解決を保証しない | 中〜大。Bevy 0.19はDraco、mesh quantization、meshopt compression等が未対応。`optimize`の汎用例をそのまま使わない。[meshoptimizer](https://github.com/zeux/meshoptimizer)、[対応表](https://docs.rs/bevy_gltf/0.19.0/bevy_gltf/#supported-khr-extensions) |
| A08 | bevy_hanabi 0.19: P2、新規装飾のみ | Bevy 0.19対応表を確認。大量のambient GPU粒子が必要なら候補 | 中〜大。Dreamはpool・128粒子benchmark・獲得量ledgerが既存。意味的な獲得処理までGPU粒子へ移さない。[公式](https://docs.rs/crate/bevy_hanabi/latest)、[Dream仕様](../dream-visual.md) |
| A09 | bevy_mod_outline 0.13: P2、仕様条件付き | Bevy 0.19対応表を確認。新しい3D輪郭が必要なら自作実装と比較 | 中。Scene RtT・RenderLayers・深度・DPI・追加pass予算を実機確認。現アートへの一律追加は勧めない。[公式](https://github.com/komadori/bevy_mod_outline) |
| A10 | **restic等の原本バックアップ: P1、既存有無確認から** | 仕様にある「別経路の日次バックアップ」を具体化し、原本・license・manifestの復元を可能にする | 小〜中＋保存先運用。現在の外部バックアップ有無は未確認。検証jobの無期限保管とは分ける。復元試験が採用条件。[restic](https://restic.readthedocs.io/en/stable/010_introduction.html) |
| A11 | Syncthing維持 / Git LFS一括移行: 保留 | 原本と承認済exportsの既存共有経路を継続。Gitとの版管理要求やclone容量問題が出ればLFS比較 | 中〜大。Syncthing versioningは他端末由来の変更が対象で、バックアップを代替しない。[versioning](https://docs.syncthing.net/users/versioning.html)、[LFS](https://git-lfs.com/) |
| A12 | Blender / hardened MCP / Khronos / 既存画像変換: 維持 | staging→実bytes検査→承認→promotionの既存工程を活用 | 大。生成サービスへの切替で受入契約は消えない。TRELLIS.2等は新規asset要求・必要GPU・生成後の手修正費用を確認してから。現PCへの標準導入は勧めない。[既存工程](../blender-setup.md)、[TRELLIS.2要件](https://github.com/microsoft/TRELLIS.2#-installation) |
| A13 | Tracy / RenderDoc / Capture→Memory runner: 維持 | GPU実体・draw/pass構造・allocatorの異なる証拠を既存経路で取得する | 新規profilerやmicrobenchmarkでは置換できない。RenderDocをframe性能の測定器として扱わない。[Tracy](https://github.com/wolfpld/tracy)、[RenderDoc](https://github.com/baldurk/renderdoc)、[既存契約](../performance-profiling.md) |

フォントはPNGに次ぐディスク量だが、単純なsubset化は日本語・記号・将来の言語・可変入力を欠落させ得る。
まず実際のロード対象とfallbackを[fonts.md](../fonts.md)で照合する。フォント削減率やVRAM効果は本調査から推定しない。

## 7. 配布と大規模置換

| ID | 候補と判断 | 適用先と期待する効果 | 負担・条件・公式資料 |
| --- | --- | --- | --- |
| X01 | cargo-about: P1、配布前 | dependencyのライセンス一覧を配布物に生成。denyの方針検査とは役割が異なる | 小〜中。crate以外のfont・画像・モデル原本の出典は既存manifestと別途集約。生成物を確認し対象releaseへ対応付ける。[公式](https://github.com/EmbarkStudios/cargo-about) |
| X02 | cargo-dist: P2、配布対象決定後 | release archive・checksum・installerの作成を自動化 | 中〜大。game executableと承認済assetsの同梱、asset検索root、保存先、OS依存、dynamic linking無効のrelease構成を検証。生成CIのbuild経路にもdriver契約が必要。[概要](https://axodotdev.github.io/cargo-dist/book/)、[追加ファイル](https://axodotdev.github.io/cargo-dist/book/reference/config.html#include) |
| X03 | Trunk: 維持、WASM限定 | `Trunk.toml`がある既存optional web経路を維持 | nativeの標準入口に置き換えない。WASM配布を再開する場合、filesystem保存・入力・rendererのtarget差を別途検証。[公式](https://github.com/trunk-rs/trunk) |
| X04 | Bevy→Godot等: 保留 | editor中心の制作要求が将来の障害になれば、別prototypeで比較する余地 | 極大。ECS Relationships、schedule、save、独自材質、Scene RtT、native verifierまで再実装に近い。現在はこれを正当化する効果測定がない。[Godot機能](https://godotengine.org/features/)、[現crate境界](../cargo_workspace.md) |
| X05 | Docker / Nix等による開発環境全面置換: 保留 | 再現しない依存環境の問題が具体化した際の比較領域 | 大。現在はtoolchain固定・OS手順・storage/lane guardがある。GPU/display/allocator受入、host共有cache、Blender環境まで含めた追加運用の費用を先に測る |
| X06 | networking / mod / embedded database基盤: 保留 | multiplayer・mod API・大量履歴検索等の製品要求が成立した時に候補を選ぶ | 大。現在の単体simulationとファイルsaveを、未要求のTokio・ネットワーク同期・DBへ移行する根拠はない。現行save/rollbackの契約を基準に再評価 |

## 8. 先行作業と採用条件

### Wave 1: runtimeを変えずに開発の抜けを補う

1. D01＋D03: cargo-denyの対象方針と依存更新PRを整える。まず既存graphで検出結果を分類し、理由のない一括ignoreを作らない。
2. D05＋D06: rootのRuff限定ルールとactionlint。doctorのbootstrap性を残し、通常gateの追加時間を記録する。
3. D04＋T01: 未使用依存は人が確認して削除。不変条件テストは1領域から始める。
4. A04＋A10: PNG数枚の比較と原本バックアップの現況確認。画質不変・復元成功を条件に次へ進む。

### Wave 2: 保存先・計測・テストの個別改善

保存先のOS対応と設定書き込みの改善は、旧データを残す移行契約を先に設計する。
nextest / llvm-covはdriverのguard対応後に限定導入し、既存方式と比較する。
Tracy・heaptrack等は観測した問題の原因を絞るために使用する。

### Wave 3: 製品要求・実測が成立したものだけ

音声mixing、再binding/gamepad、多言語、GPU粒子、texture圧縮、release packagingはそれぞれの要求を起点にする。
入力・UI・save・AIの全面置換は、局所導入では解けない理由を説明できる場合に限り再評価する。

| 比較対象 | 最小の採用判定 |
| --- | --- |
| 静的検査 | 実際の違反または意図したnegative fixtureを検出でき、誤検知の扱い・実行時間・更新担当が明確 |
| proptest | 例: 同一batchの重複assignmentやcancel/despawn後の予約について、許可された中間状態を考慮した不変条件を検査できる。縮小例を通常testで再現可能 |
| nextest | 同じfeature/対象testで結果一致、doctestを別途実行、並列数固定、全工程の時間・RSS・生成量を比較。実行時間だけ短くてもbuild増大を含める |
| llvm-cov | pure coreの未試験経路を具体的な回帰testへ結び付けられる。レビュー用cacheを変更・削除せずに運用できる |
| 保存先 | 旧相対pathの発見・移行、両方存在時の優先順位、書込不可、テストroot注入、失敗時の旧データ維持を確認 |
| oxipng | stagingの代表画像でbyte削減を測定し、decoded RGBA・metadata・ゲーム内表示を確認。効果が小さければ量産へ進めない |
| cache / profiler | 同じsource・toolchain・feature・profileで比較し、測定対象と計測overheadを区別。review cacheの破壊や大きい追加保存量を含めて判断 |
| 原本バックアップ | 独立した保存先から原本・manifestを実際に復元し、hash・参照関係が一致 |

## 9. 影響・リスク・費用

本提案の採用は一括ではなく項目単位で行う。初期候補はローカル実行できるものを中心とし、新規有料サービスを前提にしない。
費用はtool自体だけでなくCI実行、cache、バックアップ先、更新担当、失敗時の再調査を含めて見積もる。
ホスティングやActionの利用枠・料金は今回比較しておらず、契約時に実構成で確認する。

| リスク | 方針 |
| --- | --- |
| 追加依存によるcompile/upgrade負担 | 小さい境界・dev-dependency・optional featureを選ぶ。未使用toolは常用gateへ増やさない |
| Bevy互換表だけで採用 | Rust 1.96.1・Bevy 0.19・wgpu・feature構成でcompileと動作を確認。crate名のversionとBevy versionを混同しない |
| 保存互換・seedの変化 | RON serializer・乱数・WFC更新を一般cleanupと分離。既存saveと固定seed corpusを比較し、旧ファイルを上書き移行しない |
| toolが独自にcompile/clean | driverへ作用を登録してから試行。frozen subjectと他consumerのcacheを守る |
| 描画最適化による別の問題 | pass数、transparent sorting、GPU資源、画質を個別に確認。ファイルサイズをframe timeやVRAMと同一視しない |
| 検査の形骸化 | baseline丸ごとのignore・snapshot自動承認・毎回retryでflakyを隠す運用を避ける |
| 既存外部運用の見落とし | PATHにない、repoに設定がない、外部環境にもない、を区別する。backupやGitHub設定は導入前に確認 |

## 10. 検証とロールバック

実装へ進む場合はprimaryの[validation workflow](../development-infra/validation-storage-workflow.md)を読み、
対象・owner・consumer・出力・release_whenを決める。通常Cargo cacheと専用計測jobの寿命を分ける。
実機が必要な項目では`hell-workers-run-native-acceptance` Skillの既存launcher・Capture→Memory・独立verifierを使う。
計測を新toolで行っただけでnative受入済みとはしない。

各導入は独立した変更として、focused test、`python3 scripts/dev.py check`、Clippy、`python3 scripts/dev.py verify`を通す。
描画・入力・保存先のplayer契約が変わる場合は対応する仕様書とHelpを更新する。
失敗時は導入項目の設定・依存・adapterだけを戻せる境界にし、旧save・原本・承認済assetを保全する。
他sessionの差分やreview-active cacheを巻き込む一括revert/cleanは行わない。

今回の調査結果と未実施範囲:

- manifest/lockfile・現行docs・主要source・公式一次資料を確認。候補pluginをこのworkspaceへ入れた互換性test、速度比較、脆弱性監査は未実施。
- 新規native/performance job、candidate checkout、binary copyは作成していない。既存review環境は維持。
- `dev.py docs --write`: pass。root・提案索引を更新し、plan索引は変更なし。文書リンク検査もpass。
- `dev.py verify`: **fail（既存のhygiene違反）**。`scripts/validation_storage.py`はshebang付きだがGit modeが100644で、規約の100755と不一致。調査開始時のcleanなHEADから同じ状態であり、本提案では変更していない。Rust gateへ進む前に停止したため全体passとは扱わない。
- 停止までにPython 137＋151件、perf self-test、storage check、agent rules、Help impact検査はpass。crate境界検査は別途pass。
- `dev.py check`: pass（fmt・Clippy抑制規約・agent rules・workspace compile）。`dev.py cargo -- clippy --workspace --all-targets -- -D warnings`: pass。
- rust-analyzer MCPは`diagnostics: null / Unexpected response format`を返し、診断0件とは判定できなかった。上記compile/Clippyを代替確認とする。通常/profilingのRust testとprofiling各feature checkは、今回の停止したverifyでは未実施。
- 最終`dev.py docs --check`、`git diff --check`、`dev.py validation check`はpass。新規専用検証領域の保持・撤去は0件、既存保持対象は4,881,321,984 Bで開始時から不変。
- Help impact: **No impact**。変更は提案と索引のみ。runtime Helpはrootのtyped catalog/providerから構築され、`docs/`を読まない。入力・表示・保存・設定・到達可能性の実装変更はない。

## 11. 未解決事項

- 実際のCI時間内訳、テスト時間・peak RSS、cache hit率。nextest / sccache等の効果はこの測定まで未確定。
- GitHubの依存アラート設定、原本の外部バックアップと復元手順。設定がrepoにないことだけで不在とは判定しない。
- 配布OSと配布方式、セーブのportable mode、多言語・gamepad・音声の製品優先度。
- Bevy 0.19.1の個々の修正が現在の観測問題に関係するか。releaseの存在確認だけで更新必須とはしない。

## 12. AI引継ぎメモ

- 現在地: 5項目のツール導入・公開受入が完了。残る候補とruntime移行は未着手であり、採用済みとは扱わない。
- D01/D03/D05/D06/T01は[優先開発ツール導入計画](../plans/archive/priority-development-tools-plan-2026-09-13.md)に沿って実装・公開・GitHub受入を完了した。生成された依存更新PRの採用は別レビューとする。その他の候補は本評価の条件に沿って個別に計画する。
- 参照必須: [DEVELOPMENT.md](../DEVELOPMENT.md)、[invariants.md](../invariants.md)、[crate-boundaries.md](../crate-boundaries.md)、[Help契約](../help-screen.md)、[validation workflow](../development-infra/validation-storage-workflow.md)。
- 公開資料は2026-09-13確認。`latest`やGitHub mainのリンク先は変化するため、導入時には採用release/tagと依存manifestを再確認する。
- 古いasset構成・古いBevy plugin互換表・未測定の一般的な高速化率を根拠に優先順位を上書きしない。

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-09-13 | Codex | 現行構成と公式資料に基づく領域別評価、優先順位、採用条件、保留理由を作成 |
| 2026-09-13 | Codex | 最優先・優先の5項目を独立した導入計画へ接続 |
| 2026-09-13 | Codex | 5項目の導入・公開受入完了を反映し、完了計画と運用文書へ接続 |
