# Development Guide (for AI & Humans)

本プロジェクトを開発・保守する上での重要なガイドラインです。

## 開発サイクル

1.  **Planning**: 変更対象をどのクレートに置くべきか、**[クレート境界とコアロジック分離の原則 (crate-boundaries.md)](crate-boundaries.md)** に従って先に決める。crate 境界に影響する変更は `docs/cargo_workspace.md` と関連仕様書の更新範囲も同時に決める。
2.  **Execution**: 責務に合う crate で実装し、root 側は app shell と薄い互換層に保つ。初回は `python3 scripts/dev.py doctor` で環境を診断し、作業中は `python3 scripts/dev.py check` を使う。
3.  **Verification**: 修正中の見た目・操作確認には `python3 scripts/dev.py feedback` を使い、同じ作業場で差分ビルドを続ける。完了前は `python3 scripts/dev.py verify` と変更に必要な正式受入を通し、仕様変更を対応する `docs/*.md` に反映する。

## Orcaによる分離開発（Linear受付を段階受入中）

[Orca運用手順](development-infra/orca-development.md)に、条件付き編集委譲、実装2枠・専任reviewer1枠、
host重実行1枠、ticket/承認の手順をまとめた。基盤は専用branchの`d85dba0f`へローカルcommitし、
Orcaの新規treeの既定基点に設定済み。受付・統括相談・Cursor Bも`a03c4d54`へ追加commit済み。
primary未統合。受付からの統括相談・同一会話の再開は実LLMで受入済み。
実装AはCodex、実装Bは軽量task専用のCursor CLI、reviewerは固定Codex sessionとする。
同一task/sessionのworker再開と固定reviewer拘束も`074f47bc`へcommit済み（模擬provider試験・全群gate成功）。
`5abe7db6`ではA/B・固定reviewerのread-only実TUI起動/同会話再開、Cursor起動設定の分離を受入済み。
R3第1batchで対象terminal限定のread-only通信診断を`d06e912e`へcommitした（全群gate成功）。
通信成功をTask投入許可としない。
candidateではLinear snapshot adapterと受付UI、単一Dispatch bridgeのJSON内容比較・失敗照合、
Codex内側sandboxとOrca IPCの競合回避、Cursor hook bridgeを実装した。最新基盤は`85cf2843`で、
固定reviewer・Codex A・Cursor Bのread-only実Taskをheartbeat・質問再開・escalation・settlement・role終了まで一巡済み。Linearはworkspace `takumi sato` / team `TAK`の
専用試験issue `TAK-5` で作成・コメント更新・再読・worktree関連付けを受入済み。固定snapshotからの初回統括相談と
同一sessionへの追記も成功し、受付はqueued、Run/Task/Dispatchは未作成のまま維持した。
今後は[現行計画L0〜L4](plans/orca-parallel-development-plan-2026-09-20.md)に従い、
Linearを受付・進捗の正本にし、既存launcher・資源制御・固定reviewer・統括の会話再開を再利用する。
残る異常系、受付からレビューまでの一巡、編集worker受入、旧受付切替を順に受け入れる。
今回の運用基盤整備ではユーザー指定によりゲーム実装テストを実行せず、関連tooling・連携・文書/storageに限定する。
この限定は下記の通常ゲーム開発の品質規則を変更しない。詳細な対象・未実施群は計画§7で管理する。
共有checkoutと任意のbackground編集は禁止を維持する。primaryのルールは、別worktree・固定ticket・mount境界・
最大2 worker・固定read-only reviewer・統括所有の検証/commit/直列統合を満たす専用launcherだけを条件付き例外とする。
編集を伴う実装→検証→reviewは未受入であり、ルール採用だけでworkerを本番投入しない。
非ゲームのA/B専用編集fixtureと、Cursor Bの`acceptance-edit`を同fixtureだけに限定するadmissionは実装・検証済み。
[運用ガイド](orca-quickstart.md)に現在の「開発受付・統括相談」の操作を示す。
以下の既存checkoutの資源仕様を無条件に置き換えたとは扱わず、採用対象を確認してから使う。

## 開発ルール

### 1. Rust-analyzer 診断の厳守
- コンパイルエラー（赤い波線）を一つも残したまま完了報告をしてはいけない。
- `python3 scripts/dev.py check` が通ることを必ず確認する。

### 1.5. Clippy 警告ゼロの維持
本プロジェクトは Clippy で **警告0件** を目標としている（2026-03達成）。

**新規コードの規約:**

- **`type_complexity`**: `Query<...>` が複雑になる場合、以下のいずれかを選択する：
  - **Bevy システム関数の直接パラメータ**（`fn my_system(q: Query<...>)`）→ **型エイリアスを追加**：
    ```rust
    type FooQuery<'w, 's> = Query<'w, 's, (Entity, &'static Transform), With<Foo>>;
    pub fn my_system(q: FooQuery) { ... }
    ```
  - **参照パラメータ**（`fn helper(q: &Query<...>)`）→ 呼び出し側を見直し、型エイリアス付きの直接パラメータに寄せるか、必要なデータを `SystemParam` / コンテキスト構造体に再編する。
  - **`#[derive(SystemParam)]` の struct フィールド**→ フィールド型もエイリアス化・責務分割して複雑さを下げる。
  - **`ParamSet` の型エイリアス**は原則禁止。`ParamSet<'w, 's, (Query<'w, 's, ...>)>` の形で内側 Query に明示ライフタイムを付けると、`.chain()` など `IntoSystemConfigs` トレイトが壊れるため、必要なら `SystemParam` への分解や処理分割を優先する。

- **`too_many_arguments`**: Bevy system / helper 関数ともに `#[allow(clippy::too_many_arguments)]` で抑制しない。
  Query / Resource 群は `#[derive(SystemParam)]` にまとめ、純粋ヘルパーは入力構造体へ集約する。

- **`#[allow(clippy::...)]` / `#[expect(clippy::...)]` の扱い**:
  原則禁止。まずコード構造を見直して lint 自体を解消する。
  やむを得ず例外を設ける場合は、false positive か外部制約で回避不能であることを確認し、短い理由コメントと合わせてレビュアー合意を前提にする。

**Clippy 確認コマンド:**
```bash
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
```

**厳格運用ルール:**

- `python3 scripts/dev.py check` だけで完了扱いにしない。完了前に `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings` を必ず通す。
- `#[allow(clippy::...)]` / `#[expect(clippy::...)]` を追加して警告を黙らせる修正は、構造的な解消が不可能と確認できる場合を除き不可。
- 既存コードに allow が残っているのを見つけた場合も、その場しのぎで追従せず、除去できる設計に寄せる。

**lint 解消の基本方針:**

| 警告 | 現状 | 推奨する解消方法 |
|:---|:---|:---|
| `too_many_arguments` (Bevy system) | `SystemParam` / 処理分割で解消 | `SystemParam` struct にまとめる（`#[derive(SystemParam)]`） |
| `too_many_arguments` (helper fn) | 入力構造体で解消 | 引数をデータ構造 (`struct`) にまとめる |
| `type_complexity` (参照パラメータ) | 呼び出し側の再編で解消 | システム関数から直接渡すか、`SystemParam` / コンテキスト struct 化する |
| `type_complexity` (SystemParam struct フィールド) | 型整理で解消 | 型エイリアス化、責務分割、小さな `SystemParam` への分離 |

具体的な対象ファイルを調べるには：
```bash
python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings
# allow / expect は結果が空でなければ品質ゲート失敗
! rg -n '#\[(allow|expect)\(clippy::' crates --glob '*.rs'
```

### 1.6. Local 品質ゲート

#### 修正中のビルド

`python3 scripts/dev.py feedback` は既存dev profile（workspace opt=0、依存opt=2）で起動する。
`--build-only` ならビルドだけ、`-- <引数>` はゲームへ渡す。profiling featureを固定して
診断用の描画経路も含め、直接起動するnative helperと共有できるようdynamic linkingを無効にする。
`CARGO_INCREMENTAL=1` を明示し、親shellに0が残っていても無効化されない。
通常は `target/debug`、対話lane内では同じlaneの `debug` を再利用する。
専用profile・jobごとのCargo target・binary copyを作らない。初回やfeature/toolchain変更時は追加ビルドが必要。

壁・ドアの合同storyboardはnative Skillの `wall_door_joint_acceptance.py plan --feedback`
で同じdev binaryを使う。dirty sourceを許し、6画面と状態遷移を確認する。正式fixed auditは省略する。
この結果はfeedback専用で、性能比較・品質/DPI matrix・正式受入の代替にはならない。
未承認ArtPreviewのgallery等、まだ軽量経路のない専用recipeは既存helperを使う。
正式確認へ進むときだけprofiling buildと必要な監査を実施する。詳細はnative Skillを参照する。

2026-09-13の既存cacheを使った実測では、feedback初回4.49秒、未変更再実行0.40秒、
main.rsのwindow title 1行変更3.29秒、復元後2.64秒（いずれもdriver込みの実時間）。
診断変更は撤去済み。依存cacheがない初回や広範囲のcrate変更の速度は未測定で、全変更の改善率ではない。
合同feedbackはIntel Arc / Vulkan / X11で6画面・状態遷移を検証し、正式verifyがfeedbackを拒否することも確認した。
描画前の単色画面ではACKせず、画面検証が成功するまで既存120秒期限内で再撮影する。
試行2件を台帳で結果確定・finalizeし、不要job計12,062,720 bytesを撤去した。通常debug cacheは継続利用する。
同日の `check` / `verify`（Clippy警告0・全Rust test・Python tooling 137件＋Blender tooling 151件）、
native関連self-test 9件、Skill同期・ルール・storage検査はpass。
Help影響はNo impact: 開発用build・診断・保存管理のみの変更で、通常プレイヤー操作・状態・asset・
`build_help_panel_content`から生成される静的Help内容は不変。

`rust-toolchain.toml` は Rust `1.96.1` と `rustfmt` / `clippy` を固定する。rustup 環境ではワークスペースルートで cargo を実行すれば自動的に選択される。ローカルとCIは同じdriverを使い、コマンド列の乖離を防ぐ。

```bash
# 必須/任意ツール、mold、Rust toolchain、assetsをread-only診断
python3 scripts/dev.py doctor

# 日常の高速ゲート
python3 scripts/dev.py check

# 変更内容に応じた完了検証（比較基点は意図したfull commit SHA）
python3 scripts/dev.py ci check --base <full-SHA> --mode auto

# 分類を使わない全ゲート
python3 scripts/dev.py verify
```

`ci check` は比較基点とのmerge-base以後のcommitと、staged・unstaged・未追跡の差分を合算する。
検証前後のHEAD・index・source fingerprintが変われば失敗する。`--mode full`は全群を選ぶ。
比較基点を解決できない場合は、検査を省略せず失敗する。単群の切り分けは
`python3 scripts/dev.py quality --group contracts|tooling|rust|deps`を使う（完成証拠の代用ではない）。

| 群 | 検査内容 | 主な選択条件 |
| --- | --- | --- |
| contracts | storage、AIルール、Help impact、repo hygiene、crate依存、docs/index、Clippy抑制、diff hygiene | 全変更・空差分でも必須 |
| tooling | 固定版Ruff/actionlint、Python・Blender tooling test、perf self-test | Python/tooling、Rust変更 |
| rust | fmt、workspace check、profiling/通常workspace test、memory/tracy/renderdoc最小feature、Clippy警告0 | Rust source・snapshot・regression |
| deps | 固定版cargo-denyによるonline依存監査 | Cargo/lock・品質制御・設定等の全群対象 |

通常のMarkdown/README変更はcontracts、tooling変更はcontracts+tooling、Rust変更は
contracts+tooling+rust。Rust fixtureを読むtooling testがあるためRust変更でもtoolingを外さない。
Cargo/build、CI、driver・gate、AIルール/Skill、開発運用ガイド/plan template、runtime asset、未知pathは全群を選ぶ。
分類の正本は`scripts/ci_scope.py`。renameは旧名の削除と新名の追加として両方判定する。
`verify` は固定版toolを先に検査し、同じ群をcontracts→tooling→deps→rustの順にすべて実行する。
`ci check`のdiff hygieneはcommit区間・dirty・未追跡fileを検査する。CIではplanの全区間を検査する。
従来の`verify`もHEADのdirty・未追跡差分と、指定された`HELL_WORKERS_DIFF_BASE`の区間を検査する。

独立した変更は目的別branchを作り、同じ目的の修正では再利用する。共有branchを切り替える前に並行作業を確認し、
必要ならworktreeを分ける。docs正本はprimaryの`docs/`で管理する。公開が承認されたPRが自動CIの入口となる。
完了報告では、意図したbase/head/tested SHA・必要群が一致するCI URLと結果、またはローカルの
`ci check`対象・結果・fingerprintを示す。CI後にdirty変更を追加した場合や証拠対象が違う場合は再検証する。
CI未利用時はローカル`ci check`、分類を信頼できない場合はfullまたは`verify`へ戻す。
Help実レビュー、必要なnative受入、primary storage整理はCI成功と独立した義務である。
通常のcheck/buildは暗黙のログ作成や`target/`削除を行わない。容量整理は専用maintenance
scriptを明示的に実行する。検証用job・binary copy・worktreeは
[検証データ管理](development-infra/validation-storage-workflow.md)に従い、成功・失敗・中断を問わず
各バッチの報告前に結果を確定・整理する。全jobの証拠保存は要求しない。担当者の必須手順であり、track closeまで
一括保持しない。ただしフィードバック対応中のcandidate worktreeとtargetは同じ場所に維持して
差分ビルドへ使う。レビュー待ちをconsumerとして記録し、出力整理と作業環境撤去を分ける。
残存は所有者・具体的consumer・bytes・次の作業・終了条件をprimary計画へ記録する。
実行はprimaryの `python3 scripts/dev.py validation` へ登録し、plan/execute、seal/finalize/checkを通す。
`seal`は独立検証の結果だけを台帳に記録し、capsuleを作らない。`verify`も未整理・用途のない残存を検査する。
容量上限と保存日数の既定値はなく、任意のreview_atは状態確認用で修正buildを停止しない。
旧凍結helperはprimary coordinatorの子processで実行し、
凍結worktreeへ新規則をコピーしない。詳細とJSON specは上記の検証データ管理を参照する。
`scripts/dev.py`が起動するCargoは、親shellの`CARGO_TARGET_DIR`、
`CARGO_BUILD_TARGET_DIR`、`CARGO_BUILD_BUILD_DIR`、`TMPDIR`、`CARGO_HOME`、`RUSTUP_HOME`を
安全な永続領域へ正規化し、通常はworkspace `target/`、`target/.dev-tmp`、既定のaccount
toolchain cacheへ固定する。2窓の対話作業では各ターミナルで`python3 scripts/dev.py lane shell`
を開始し、`target/lanes/a`または`target/lanes/b`をshell終了まで固定する。lane内のbuild
jobは1、laneなしは従来の安全計算（最大2）とし、2窓の合計compile fan-outを2に抑える。
incrementalは各runnerの用途に応じて明示する。Cargo compilationを伴うgateは`MemAvailable`
8 GiB未満では開始前に停止する。swapの使用量はmanifestへ診断情報として記録するが、RAMの
下限を満たす場合の開始条件にはしない。Linuxでは`/proc/meminfo`の`MemAvailable`を読めない
場合に開始前に停止する。
対話Cargoのcompile / run / test / clippyは `target/.cargo-activity.lock` のshared leaseを
Cargo childの生存中だけ保持する。performance runnerとnative acceptance recipeは同じlockの
exclusive leaseをrecipe全体で保持し、競合時はchildを起動せず明確なbusy理由で停止する。
lane leaseはsession所有、activity leaseは実行中資源の所有であり、idle lane shellだけでは
native/performanceを妨げない。
**このcheckoutでは、compile/run/test/clippyのためにraw Cargoを実行しない。** 追加のCargo subcommandも
`python3 scripts/dev.py cargo -- <subcommand> ...`を使う。品質ゲートをraw Cargoへ置き換えたり、`/tmp`
target・cache・成果物を指定したりしない。

### 2. 死蔵コードの禁止 ([deadcode.md])
- 将来使う予定があっても、現在使われていないコードや `#[allow(dead_code)]` は残さない。

### 3. 画像生成と透過 PNG ([image-generation.md])
- アイコン等は `generate_image` で背景をマゼンタ (`#FF00FF`) にして生成する。
- `scripts/convert_to_png.py` を使用して透過 PNG に変換する。
- 変換後はバイナリ署名を確認する： `89-50-4E-47-0D-0A-1A-0A`
- 複数 PC 間で原本を共有する場合は、`Syncthing` でリポジトリ外の原本フォルダを同期し、`exports/` から `python scripts/sync_external_assets.py --source ~/Sync/hell-workers-assets/exports` で `assets/` に反映する。詳細は `docs/assets_workflow.md` を参照。

### 4. 型変更とメッセージ初期化の規約
型不一致や二重借用エラーが長引きやすいため、以下を必ず守る。

- 型変更の順番は `定義 -> 生成 -> 使用` を固定する
  例: `entities` の `struct/enum` を更新してから、`spawn/build` 側、最後に `systems` の `Query` を更新する。
- 変換は `From/Into` に統一し、`as` の多用を避ける
  変換地点を明確にして、型ミスの原因位置を特定しやすくする。
- `Messages<T>`/`Events<T>` は専用プラグインで集中初期化する
  `crates/bevy_app/src/plugins/messages.rs` などに集約し、`build()` 冒頭で `add_message::<T>()`/`add_event::<T>()` を登録する。
- 初期化漏れに備えて `Option<Messages<T>>` か `If<Messages<T>>` を検討する
  使わないフレームでもパニックしない形にしておく。

### 5. EntityEvent Observer 登録の規約
- `EntityEvent` のオブザーバーは、原則として Plugin 側の `app.add_observer(...)` に一元登録する。
- 同じハンドラをスポーン時の `.observe(...)` と併用しない（重複実行の原因になる）。
- 例外として、特定エンティティにのみ限定した監視が必要な場合に限り `.observe(...)` を使う。

### 6. 予約（Reservation）実装の規約
物流・自動補充の競合を防ぐため、予約の責務と解除タイミングを明確にする。

- 予約責務は「発行時」か「割り当て時」のどちらか一方に統一する。
- 自動発行・割り当ての排他は `ResourceReservationOp` と `SharedResourceCache` の reservation snapshot で表現する。component marker を予約根拠に追加してはならない。
- 共有ソースを消費するタスク（例: Tank からの取水）は、処理中に `ReserveSource` でロックし、成功/失敗/中断の全経路で解除する。
- フェーズ移行で不要になったロックは即時解除し、`unassign_task` 側でもフェーズに応じて解放漏れを防ぐ。
- `sync_reservations_system` の再構築条件は、実行フェーズの予約寿命と一致させる（フェーズ定義を変更したら同時更新する）。比較用の `ReservationSignature` は `collect_active_reservation_ops` / `active_reservation_signature` と同じ正規化経路から導出し、独立した phase match を増やさない。progress のような予約非依存フィールドだけで snapshot を再構築しない。
- `SharedResourceCache` の frame-local delta と reservation snapshot は別の寿命で管理する。snapshot 置換で未反映 delta を消さず、Entity を持つ reservation signature cache と、その再構築を保証する同期 timer は load 時に必ず reset する。

### 7. TransportRequest の規約（M3〜M7 完了）
運搬系は全て **Anchor Request パターン** に統一済み。request エンティティをアンカー位置（Blueprint/Mixer/Stockpile）に生成し、割り当て時にソースを遅延解決する。

- **request 化済み**: `DepositToStockpile`, `DeliverToBlueprint`, `DeliverToFloorConstruction`, `DeliverToWallConstruction`, `DeliverToProvisionalWall`, `DeliverToMixerSolid`, `DeliverWaterToMixer`, `GatherWaterToTank`, `ReturnBucket`, `ReturnWheelbarrow`, `BatchWheelbarrow`, `ConsolidateStockpile`, `DeliverToSoulSpa`
- 通常producerは`crates/hw_logistics/src/transport_request/producer/`に置く。`DeliverToSoulSpa`だけはroot固有の建設siteとorderingを扱うため`crates/bevy_app/src/systems/jobs/soul_spa_construction/auto_haul.rs`が生成する。
- `task_finder` は `DesignationSpatialGrid` と `TransportRequestSpatialGrid` の両方から候補を収集。
- 運搬系 WorkType（`Haul`, `HaulToMixer`, `GatherWater`, `HaulWaterToMixer`, `WheelbarrowHaul`）は request 付き候補のみを扱う。
- request は需要 0 のとき `Designation` を外して休止、または despawn。
- アンカー消失時は `transport_request_anchor_cleanup_system` で request を close。

### 8. 割り当て・搬送・UIの実装境界

- Familiar の割り当て発行は `crates/hw_familiar_ai/src/familiar_ai/decide/task_management/builders/submit.rs` の `submit_assignment_with_source_entities(...)` / `submit_assignment_with_reservation_ops(...)`（または下位の `submit_assignment(...)`）を必ず経由する（`ReservationShadow` 反映を保証するため）。
- 予約オペレーション生成は `build_source_reservation_ops` / `build_mixer_destination_reservation_ops` / `build_wheelbarrow_reservation_ops` の共通ヘルパーを優先し、`issue_*` ごとの重複実装を増やさない。
- `FamiliarTaskAssignmentQueries` は必要な Read Access を内包する構成になっている。Familiar 側の型参照は `task_management::FamiliarTaskAssignmentQueries` を優先し、`soul_ai` 実装詳細への直接依存を増やさない。
- `apply_task_assignment_requests_system` を拡張する場合は、既存の責務分離ヘルパー（受理判定 / idle正規化 / 予約反映 / DeliveringTo / イベント）へ追記し、単一関数へ責務を戻さない。
- `pathfinding_system` の変更は補助関数（再利用判定・再探索・休憩フォールバック・失敗時処理）単位で行い、分岐をインラインで肥大化させない。
- floor/wall の搬入同期変更は `crates/hw_logistics/src/transport_request/producer/mod.rs` の共通ヘルパー（`sync_construction_requests`, `sync_construction_delivery`。内部で `group_tiles_by_site`, `consume_waiting_tile_resources` を利用）を再利用して重複実装を避ける。
- UI/Visual の更新は `crates/bevy_app/src/interface/ui/interaction/status_display/` と `crates/hw_visual/src/dream/ui_particle/` の責務分割単位で行い、再び単一巨大ファイルに戻さない。
- UI buttonの`Changed<Interaction>`は`ui_interaction_system`が読み、`ForegroundUiGate`通過後に`UiIntent`を発行する。ゲーム副作用はrootの単一`handle_ui_intent`が適用する。`MovePlantBuilding` / `ToggleDoorLock` / `SelectArchitectCategory`向けの直接Interaction consumerを追加しない。

### 9. docs 直下ドキュメントの記述ルール

#### 9.1 基本方針

- `docs/*.md`（`plans/` と `proposals/` を除く）は、作業報告ではなく仕様・設計・運用ルールの説明を目的とする。
- 「対応済み」「実装完了」「今回の変更」など、時点依存の進捗/報告表現は書かない。
- 実施ログ・進捗・作業メモは PR 説明、Issue、または `docs/plans/` / `docs/proposals/` に記載する。
- 挙動変更を伴う実装時は、関連する仕様文書と `docs/README.md` の参照関係を同時に更新する。

#### 9.2 更新トリガー（実装と同時に更新する）

以下のいずれかが変わったとき、該当ドキュメントを必ず更新する:

| 変更内容 | 更新対象 |
|:---|:---|
| 新しい Relationship/コンポーネントを追加 | 書き込み元・削除元・非自明な挙動を接続マップ表に追記 |
| 既存コンポーネントの書き込み元/削除元が変わる | 接続マップ表の対応行を修正 |
| task_finder フィルタ条件が変わる | `tasks.md §3` の発見性チェックリストを更新 |
| `unassign_task` の契約が変わる | `tasks.md §5` を更新 |
| Observer/イベントチェーンが変わる | `tasks.md §4.4` のイベント表を更新 |
| 新規 WorkType / TransportRequest 種別を追加 | `tasks.md §4.3` と `logistics.md §3` を更新 |
| サイレント失敗条件が追加/変更される | ⚠️ マーカー付きで記載（エラーなしでフィルタされる条件） |
| TaskMode バリアントが変わる | `state.md` のバリアント一覧を更新 |
| 空間グリッドを追加/削除 | `architecture.md` のグリッド一覧を更新 |
| crate の責務や定義場所が変わる | `cargo_workspace.md` と関連仕様書の参照先を更新 |
| 移設済み system の登録責務や ordering が変わる | `architecture.md` / `cargo_workspace.md` / 関連仕様書に「唯一の登録元」と ordering 契約を追記 |
| player-facing feature/input/UI label/workflowを追加・変更・削除 | `help-screen.md`の手順でmanifest/provider/coverageを更新するか、変更バッチへ理由付きHelp no-impact判断を残す |

#### 9.2.1 Help impact contract

`python3 scripts/dev.py verify`は`scripts/check_help_impact.py`で、diff base以後のproduction変更
（test専用fileを除く`crates/*/src/**/*.rs`、Cargo/build、repository-owned runtime text data）を一つの
change batchとして検査する。root所有のHelp catalog更新、または`Help-Impact: none`と空でない
`Help-Impact-Reason: ...` trailerを持つcommitが、commit DAG上で全production commitの子孫にある必要がある。
判断後のdirty変更や別branchのproduction変更を追加した場合は、新しいHelp更新または判断が必要になる。
Help更新として数えるには`help_content/`配下のproduction Rustまたはsnapshot対象のtyped renderer
`hw_ui/src/help.rs`と、`coverage_approval.snap`のexact snapshotを同じ判断commitまたはdirty batchで
更新する必要がある。test、README、fixture、sourceだけ、
approval snapshotだけではproduction batchを承認できない。merge commitは親差分のunionを再計上せず、
Gitの自動merge treeから外れたresolution/追加編集だけをmerge固有変更として判定する。
この再構築はGit標準の2-parent mergeを対象とする。octopus mergeまたは自動treeを再構築できない履歴では
gateをfail-closedで停止し、標準と異なるmerge strategy/optionによるtree差分は安全側にmerge固有変更として扱う。

dirtyなローカル検証だけは`HELL_WORKERS_HELP_IMPACT_REASON`を使用できるが、CIでは無効である。CIは
non-zeroかつ解決可能で`HEAD`とmerge-baseを持つ`HELL_WORKERS_DIFF_BASE`を必須とする。rename、
staged/unstaged、未追跡fileも検査する。localizationやdata-driven command/label用に新しいsource root/拡張子を
追加する場合は、gateのpath分類とfixtureを同じ変更で更新する。ローカルでも`origin/master`とのmerge-baseを
解決できない既存履歴では`HEAD^`へ縮退せず、fetchまたは`HELL_WORKERS_DIFF_BASE`の明示を要求してfail-closedにする。
詳細なcatalog ownershipと追加/削除手順は[help-screen.md](help-screen.md)を参照する。

#### 9.3 記述内容の優先順位（MCP-aware 原則）

`rust-analyzer-mcp` / `docsrs-mcp` で取得可能な情報はドキュメントに書かない。**ドキュメントにしか書けない情報を優先する。**

**書くべき内容（MCP では追いにくい ECS 疎結合）:**
- Relationship の「書き込み元」「削除元」（どのシステムが insert/remove するか）
- 複数システムにまたがる副作用と依存順序
- 「何をしない」契約（呼び出し元の責務として残されているもの）
- サイレント失敗トラップ（フィルタされてもエラー/ログが出ないケース）
- グリッド同期の Change Detection 遅延（スポーン後の次フレームで反映）など、タイミング依存の挙動

**書かないべき内容（MCP で参照可能）:**
- struct のフィールド一覧（型・説明）
- enum のバリアント一覧（自明な名前のもの）
- `crates/bevy_app/src/` ファイルパスの羅列
- 定数の数値テーブル（`crates/bevy_app/src/constants/` に集約済み）
- Mermaid フローチャート（同等のテキスト表現で代替できる場合）

#### 9.4 Relationship の記述形式

ECS Relationship を追記する際は **tasks.md §2.1** と同じテーブル形式を使う:

```markdown
| Source（手動操作）| Target（Bevy自動）| 書き込み元 | 削除元 |
|:---|:---|:---|:---|
| `FooRelation(bar)` ← entity | `FooBars` ← bar | `apply_xxx_system`（Execute）| `unassign_task` / タスク完了 |
```

- Source 側のみ手動操作し、Target 側は Bevy が自動更新する旨を冒頭に明記する。
- 複数システムが削除する場合は全て列挙する。

### 10. MCP（rust-analyzer-mcp / docsrs-mcp）活用フロー

- 目的:
  - `rust-analyzer-mcp`: ローカルコードの型・参照・定義を正確に把握する。
  - `docsrs-mcp`: 外部 crate API（特に Bevy 0.19）のシグネチャと仕様を一次情報で確認する。
- 実装前:
  - 変更対象シンボルは `rust-analyzer-mcp` で定義・参照・関連型を確認する。
  - 外部 API を使う箇所は `docsrs-mcp` で対象バージョンの関数シグネチャを確認する。
- 実装中:
  - ローカル依存関係の追跡は `rust-analyzer-mcp` を優先する。
  - API 仕様確認は `docsrs-mcp` を優先し、推測でメソッド名や引数を書かない。
  - Bevy API は必ず 0.19 系のドキュメント/シグネチャで確認する。
- 実装後:
  - rust-analyzer 診断を確認し、`python3 scripts/dev.py check` を必ず実行する。
  - MCP の結果と実コードが不一致の場合は、`~/.cargo/registry/src/` の実ソースを確認して整合を取る。
- MCP が使えない場合の代替:
  - `~/.cargo/registry/src/` のクレートソースと `docs.rs` の一次情報で確認する。

### 10.4 Repository Agent Skills

- リポジトリ直下`.cursor/skills/<name>/SKILL.md`を共有Skill本文の正本とする。現行Skillは
  `hell-workers-update-docs`、実装後にplayer-facing Help影響を判定する
  `hell-workers-review-help-impact`、no-prompt launcherで実機受入を実行する
  `hell-workers-run-native-acceptance`。
- 機能、code、runtime dataを実装・変更・削除した後は、完了報告・commit・publishより前に必ず
  `hell-workers-review-help-impact`を使い、実際のplayer-visible経路から`Update required`または
  `No impact`の判断を完了する。`Undetermined`のまま完了せず、Help impact gateの成功だけで代用しない。
- productがSkillをnative公開しない場合も省略せず、
  `.cursor/skills/hell-workers-review-help-impact/SKILL.md`の正本を直接読んで同じ手順を実行する。
- Codex、Gemini、Claude adapterはproduct固有frontmatterを保持し、本文だけを
  `python3 scripts/sync_agent_skills.py --write`で同期する。
- 同期前に全adapterの`name`、非空`description`、Codexの`agents/openai.yaml`にある表示名・短い説明・
  `$skill-name`を含むdefault promptを検証する。1件でも欠落・不正なら、他adapterを書き換える前に停止する。
- 新しい共有Skillを追加するときは、`.cursor`正本、全adapter、`scripts/sync_agent_skills.py`のmapping、
  `scripts/check_agent_rules.py`のactive skill一覧、同期testを同じ変更で追加する。
- `check_agent_rules.py`は全Skillの本文同期とactive rule内容を非変更検査する。

### 10.5 デバッグ規約

不具合調査では、分野を問わず以下の順で切り分ける。

- **観測事実を固定する**  
  ユーザーや検証者が報告した観測、または直前の実験で確定した事実は、以後のデバッグで否定されない限り事実として扱う。

- **調整より成立条件を先に確認する**  
  パラメータ調整や見た目調整に入る前に、対象機能の経路と前提条件が成立しているかを確認する。  
  例: 対象が同じデータ集合を見ているか、想定したイベント / レイヤ / フラグ / pass / state に参加しているか、実行順が前提どおりか。

- **仮説は 1 回で評価し、変化がなければ切る**  
  probe や一時変更を入れる時は、「この変更で何が変われば仮説が当たりか」を先に決める。変化が出なければ、その仮説はそこで打ち切る。

- **フレームワーク依存の説明は一次情報で確認する**  
  ライブラリやエンジンの挙動は推測で説明しない。`docsrs-mcp`、`rust-analyzer-mcp`、`docs.rs`、`~/.cargo/registry/src/` などの一次情報で成立条件を確認してから実装・説明する。

- **診断実装は作業完了前に必ず撤去する**  
  probe entity、debug 色、一時的な hidden / layer / flag 変更などは、原因切り分け後に消す。恒久実装に診断用経路を残さない。

### 11. 非Walkable対象の到達ロジック規約（タスク実行）

建物・岩・資源ノードなど、**対象セル自体が walkable でない可能性があるタスク**では、`Destination` に対象中心座標を直接入れないこと。

- `GoTo*` フェーズでは `update_destination_to_adjacent(...)` を使い、到達可能な隣接セルへの経路を設定する。
- 到達判定は `is_near_target_or_dest(...)`（または等価の隣接判定）を使い、対象中心距離のみで判定しない。
- `reachable == false` の分岐を必ず実装し、タスク解除・予約解放・`WorkingOn` クリーンアップまで行う。
- 新規タスク実装時は、既存の `gather` / `haul` / `refine` / `coat_wall` の `GoingTo*` 実装パターンを踏襲する。

### 12. 関数命名規則

#### 12.1 システム関数（`add_systems` で登録するもの）

`{動詞}_{対象}_system` の形式を使う。

```rust
// ✅ 推奨
pub fn update_resource_spatial_grid_system(...) { ... }
pub fn cleanup_commanded_souls_system(...) { ... }
pub fn sync_wall_tile_visual_system(...) { ... }
pub fn apply_task_assignment_requests_system(...) { ... }

// ❌ 動詞なし（何をしているか伝わらない）
pub fn resource_spatial_grid_system(...) { ... }
pub fn animation_system(...) { ... }
```

**承認済みの動詞**:

| 動詞 | 意図 |
|------|------|
| `update` | コンポーネント値の更新・UI 再描画 |
| `sync` | 2 つのデータ間の整合取り |
| `apply` | メッセージ/リクエストキューの消費・反映 |
| `cleanup` | エンティティ・コンポーネントの削除 |
| `detect` | 状態変化のスキャン |
| `spawn` | エンティティ生成 |
| `tick` | タイマーの進行 |
| `animate` | ビジュアルアニメーション更新 |
| `perceive` | AI Perceive フェーズ |
| `decide` | AI Decide フェーズ |
| `execute` | AI Execute フェーズ |
| `process` | 複数ステップの複合処理（ヘルパー化できない場合） |

#### 12.2 Observer 関数（`add_observer` で登録するもの）

`on_{イベント名}` の形式を使う。`_system` サフィックスは付けない。

```rust
// ✅ 推奨
pub fn on_task_assigned(trigger: Trigger<TaskAssigned>, ...) { ... }
pub fn on_building_added(trigger: Trigger<BuildingAdded>, ...) { ... }

// ❌ _system サフィックスを使わない（Bevy System ではない）
pub fn task_assigned_system(trigger: Trigger<TaskAssigned>, ...) { ... }
```

#### 12.3 ヘルパー関数（直接登録しないもの）

`_system` サフィックスを付けない。動詞始まりの自由形式。

```rust
// ✅ ヘルパー（Bevy に直接登録しない補助関数）
pub fn process_task_delegation_and_movement(...) { ... }
pub fn apply_door_state(world_map: &mut WorldMap, ...) { ... }
pub fn is_soul_available_for_work(assigned: &AssignedTask) -> bool { ... }
```

### 13. ワンショット遅延処理の規約（Delayed Commands vs Timer）

「N 秒後に一度だけ何かする」処理は、Timer コンポーネント + tick システムを新設せず、Bevy 0.19 の Delayed Commands（`commands.delayed().secs(..)`、`bevy::prelude` に含まれる）を第一候補にする。

**Delayed Commands を使ってよい条件（5 つすべて満たすこと）:**

1. `TimerMode::Once` 相当のワンショットである
2. 発火までの経過が**無条件**（状態によって進行を止めない）
3. 途中キャンセル・リセット・上書きの経路が**存在しない**（Delayed Commands にキャンセル API はない）
4. 発火時に必要なデータが、発行時に確定しているか、closure コマンド内で World から取得できる
5. 同じ対象へ短時間に複数回発行されても、旧実装の上書き/デバウンス挙動に依存していない（複数キューはすべて発火する）

1 つでも満たさない場合は従来どおり Timer コンポーネント + tick システムを使う（例: `ItemDespawnTimer` は relationship で保護されたアイテムの tick を止めるため対象外、`DoorCloseTimer` は近接キャンセルがあるため対象外）。

**実装上の注意:**

- 発火は `TimePlugin` が `PreUpdate` に登録済みの `check_delayed_command_queues` が担う。アプリ側の登録は不要
- デフォルトクロック基準のため、ポーズ/倍速（`Time` の pause / relative_speed）に自動追従する
- closure コマンド（`.queue(move |world: &mut World| {..})`）内では、対象エンティティの despawn に備えて `world.get_entity(..)` で存在確認する
- closure 内で resource と `world.commands()` を同時に使う場合は `world.resource_scope` で borrow 競合を回避する
- `Res<T>` は `&mut World` から再構築できない。closure から呼ぶヘルパーの引数は `&Res<T>` ではなく `&T` にする（既存の呼び出し元は deref coercion でそのまま通る）
- 適用例: `ConversationCooldown` の時限除去、勧誘/激励リアクションの遅延バブル（`docs/speech_system.md` 参照）

## 開発ツール

### Quality tools（full verifyで必須）

固定版の正本は[`scripts/dev-tools.toml`](../scripts/dev-tools.toml)で、cargo-deny 0.20.2、
Ruff 0.16.7、actionlint 1.7.12を使う。Linux x86_64では公式releaseのSHA-256を照合する
専用installerを明示実行する。通常の`doctor` / `check` / `verify`はinstallやupgradeを行わない。
全体`verify`には、画像処理testのため同じPython環境のPillow 12.3.0も必要である。

```bash
python3 scripts/install_dev_tools.py --bin-dir "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"
python3 scripts/dev.py doctor
python3 scripts/dev.py lint
python3 scripts/dev.py deps
# cached DBによる診断だけ。online監査の代用にはしない
python3 scripts/dev.py deps --offline
```

installerはLinux x86_64以外を拒否する。他hostでは同じ版の公式配布物を各toolの公式手順で
永続領域へ供給する。cargo-denyをsource buildする場合は
`python3 scripts/dev.py cargo -- install cargo-deny --version 0.20.2 --locked`を使う。
`doctor`は`RUSTUP_AUTO_INSTALL=0`でRustとtoolの版・pathを診断し、追加tool不足は
`Full verification tools: not ready`として通常build readinessと分ける。
`check`は追加CLIを要求しない。`verify`は欠損・版違いを先頭で拒否する。
Cargo homeの`bin`にあるcargo-denyがPATHより優先されるので、古い同名binaryも診断対象になる。
衝突時は診断に表示されたbinaryの親directoryをinstallerの`--bin-dir`へ指定し、
`--tool cargo-deny`でその場所の版を揃える。別directoryへ追加するだけでは衝突を解消しない。

RuffはPython 3.11向けの`scripts/`に`E4/E7/E9/F`だけを適用し、cache・自動修正・formatを使わない。
`ruff.toml`の`required-version`とmanifestの一致も検査する。actionlintはworkflowの構文・式・型を検査し、
任意導入のShellCheck/Pyflakes連携を無効にして環境差を避ける。Dependabot YAMLはactionlintの対象外である。

依存監査は全13 workspace、全feature、target filterなしのnormal/build/dev graphを対象とする。
`--locked`でlock変更を拒否し、online DB更新失敗をofflineへfallbackしない。
offlineではDB欠損・鮮度検査失敗もnon-zeroになる。DBは正規化したCargo homeの
`advisory-dbs/`、registryは同`registry/`に置き、通常開発cacheとして保守する。
監査はnative/performanceのactivity exclusive leaseと競合する間、起動しない。

`deny.toml`は未知license/source、既知脆弱性、推移依存のunsound/未保守通知を拒否する。
内部memberの`publish = false`と`licenses.private.ignore`は自前license判定だけを外し、
外部依存をgraphから除外しない。外部licenseはMIT、Apache-2.0、BSD-2-Clause、BSD-3-Clause、
BSL-1.0、CC0-1.0、ISC、MIT-0、Unicode-3.0、Zlibを許可する。配布時のnotice義務は別途維持する。
複数versionとyankedはwarningとして表示し、Clippy警告ゼロとは区別する。

例外は[`RUSTSEC-2026-0192`](https://rustsec.org/advisories/RUSTSEC-2026-0192.html)だけである。
`bevy_winit → winit → sctk-adwaita → ab_glyph → owned_ttf_parser → ttf-parser 0.25.1`
というWayland decoration経路に修正版のない保守終了通知がある。これは当該通知に脆弱性の記載がないことを
確認した個別例外であり、同crateの新しいadvisoryは抑制しない。ownerは依存更新reviewer、
next actionはwinit/sctk-adwaita更新時のfont backend移行確認、解除条件はこの依存経路の解消である。

CLI更新担当はversion・公式URL・公式release digestを同時に更新し、Ruffの要求版を同期する。
明示install後に`doctor` / `lint` / `deps` / `verify`を通す。Dependabotはこの任意TOMLの版を更新しない。

### Property tests

proptestはroot workspace dependencyで管理し、`hw_world` / `hw_infra`だけのdev-dependencyとする。
`default-features = false`、`std`だけを使い、fork/timeout subprocessを作らない。
検索のA→B→A履歴非干渉・経路合法性、光源順の不変性、照明再適用の無変更を計3 propertyで検査する。
各256 cases、縮小1024回、最大8×8・6光源で処理量を制限する。

```bash
PROPTEST_RNG_SEED=20260913 python3 scripts/dev.py cargo -- test --locked -p hw_world -p hw_infra properties
```

失敗seedは標準SourceParallelで次の2箇所へ保存され、Git・IDE ignoreの狭い例外で追跡できる。
成功時の空corpusは作らない。生成器を変えるとseedだけでは再現を保証できないため、
実際の失敗seedをレビューしてcommitし、縮小された入力を通常の`#[test]`へ昇格する。

```text
crates/hw_world/proptest-regressions/pathfinding/tests/properties.txt
crates/hw_infra/proptest-regressions/lighting/properties.txt
```

通常のmutable workspace/laneで実行する。凍結したnative subjectでは実行せず、
failure persistenceの無効化によって既存seedの再生を止めない。

### Dependency update PRとCI

[Dependabot設定](../.github/dependabot.yml)はCargo/Actionsを週次月曜に確認する。
通常version更新のopen PR上限はCargo 3、Actions 1。engine-render、worldgen、その他Cargo、Actionsを分け、
自動mergeしない。Bevy/wgpu更新はAPIと必要なnative受入、rand/WFC更新は同seedと保存互換をreviewする。
groupは`version-updates`専用で、security更新の上限やgroupを保証しない。
security更新はGitHub側の設定も必要で、有効化する場合は`security-updates`用groupを別途設計する。
default branchへの反映後、両ecosystemのscan、生成PR（またはno-update）とCIをGitHub上で確認する。

Cargo manifest/lockはdev-dependency更新でもHelp gateの対象である。bot commitだけの初期失敗は
実レビュー待ちとして扱う。担当者がplayer-visible経路を確認し、必要なHelp更新または
全更新commitの子孫となる末尾commitの`Help-Impact: none` / 具体的な`Help-Impact-Reason`を残す。
PR本文・CI環境変数・固定理由の自動注入で代用しない。bot再更新/rebase後は再判断し、squash後も判断を保持する。

CIは公式SHA固定Action、read権限で`verify`と同じ群を選択実行する。
`.github/workflows/ci.yml`はPR（draftも含む）、master push、手動実行に対応する。
手動実行は`mode=auto|full`と、対象の厳密な祖先である非zeroのfull `base_sha`が必要。
PRはeventのbase B/head H、merge-base G、実際のmerge commit Tを固定し、Tの親がB,Hかを確認して
G..HとB..Tを合算する。master pushはbefore..after、non-FFはG..before/G..afterも加えてfullにする。
Help基点はPRのB、通常pushのbefore、non-FFのG、手動のbase。取得不能なSHAや不正eventは失敗する。

`changes`がschema v1のplanを作り、各群と集約が同じeventから再計算して照合する。
成功jobはtested SHAとplan digestを出力する。digestはrun IDを含み、failed-job再実行のためattemptのみ除外する。
contracts成功後にtooling/deps、必要群成功後にrustを実行する。rustだけにnative toolとCargo cacheを供給し、
既存cache keyを維持する。toolingにはPillow 12.3.0を一時venvへ導入し、`get_flattened_data`の存在を確認する。
rustは90分上限。各群の固定版toolは`.github/actions/prepare-quality/action.yml`が必要分だけ供給する。

常設job `quality`（表示名`Quality gates`）は`always()`で、選択群のsuccess・SHA/digest一致と
未選択群のskippedを確認する。失敗・取消・選択群のskip・欠損は通過させない。
Summaryにはbase/head/tested SHA、選択理由・群、path件数/digest、結果、run URLを記録する。
branch保護はworkflowと別のGitHub設定であり、check成功だけでmerge強制とはしない。

`.github/workflows/dependency-audit.yml`は日次03:17 UTCと独立した手動実行で`deps`だけを走らせる。
game build・Help gate・`Quality gates`を作らず、CI workflowの手動fullとは区別する。
concurrencyはworkflow/event/PRまたはrefで分離し、通常CIは手動modeも分ける。
手動監査成功と実際のschedule発火は別々に受入記録へ残す。

2026-09-13の導入受入では、`ecf2ab7d`で5項目を公開し、`c870bfea`でCIのPillow供給を固定した。
[通常CI](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34748374154)は
Python147+151 test、通常/profiling構成のworkspace test、各profiling feature check、Clippy警告0件、
全契約検査とCargo cache保存まで成功した。job全体は69分02秒で、90分上限内に収まった。
[Cargo scan](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34748265575)と
[Actions scan](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34748265761)も成功し、
設定どおりCargo 3件（PR #14〜#16）・Actions 1件（PR #13）が生成された。
PR #13〜#15の初回CIは旧Pillow供給で停止し、修正後のbaseを使った#16はPython test等を通過後、
Cargo変更のHelpレビュー未記録で拒否された。初回の失敗と依存更新自体の互換性は区別する。
[日次監査と同じjobの手動実走](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34748259651)は成功。
定刻scheduleの発火自体は未観測。security updatesはdisabledのままで、alertsの有効状態はAPIの404応答では確定できなかった。
Helpへの影響はNo impact（開発環境・監査・testと互換security patchのみ、入力・保存・Help catalogは不変）。

#### 初回依存PRのレビュー（2026-09-13）

| PR | 判断 | 根拠と再検討条件 |
| --- | --- | --- |
| [#13](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/13) | 採用、merged | checkout 7.0.1の公式SHAを照合。ref判定のUnicode処理とgit config解除値のescape修正。権限やworkflow入力は維持。 |
| [#14](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/14) | 不採用、closed | Bevy 0.19.1もwgpu 29を要求する。直接依存の30への変更はprofilingのSurfaceTargetUnsafe / SurfaceCapabilities / Backendの型境界を壊す。Bevyと直接依存の系列を揃えた移行とnative受入を用意して再検討。Bevyのpatchのみの更新は分離可能。 |
| [#15](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/15) | 不採用、closed | WFC 0.10.7はrand 0.8 / direction 0.18を要求する。直接依存の0.10 / 0.19はRNG traitとPatternDescriptionの型に不一致を作り、SimulationRngの旧API移行もない。WFCとの統合移行・seed互換性検証を計画して再検討。 |
| [#16](https://github.com/iaammssssstupiddddd-commits/hell-workers/pull/16) | 互換修正を加えて採用、merged | sha2 0.11 / libloading 0.9とRON・Serde・JSONのlock更新。16進数変換を修正し、独立計算した照明checksumの固定vectorを既存testへ追加。 |

#16のHelp判断は **No impact**。SHA-256はwall/door manifestの実bytes検査と照明の同一性判定に使い、
digestの入力と32byte出力を維持する。sha2 0.11の戻り値は`LowerHex`を実装しないため、
asset loaderとprofilingの14か所を既存`hw_infra::lighting::digest_hex`へ移し、
先頭ゼロを含む64桁小文字形式を維持する。照明の固定vectorはPython hashlibの独立値に照合する。
Save/Loadは既存のheader version・DynamicWorld RON、SettingsはGameSettingsFileと既存default移行を維持し、
serde_jsonの非文字列enum key拒否修正に依存する入力を生成しない。新しいRON構文は製品で使用しない。
libloadingはLinuxのprofiling-renderdocで注入済みライブラリを`RTLD_NOW | RTLD_NOLOAD`で開き、
固定の`RENDERDOC_GetAPI`を解決する用途だけ。0.9でも`&str`/byte literal、flags、handle所有とsymbol寿命は維持される。
`Error` variantへのmatchや削除されたAPIは使わない。診断のerror chain以外に変更はなく、player入力・成立条件・
成功結果・文言を追加変更しないためHelp source/snapshotは更新しない。

一次情報: [checkout差分](https://github.com/actions/checkout/compare/v7.0.0...v7.0.1)、
[Bevy manifest](https://docs.rs/crate/bevy_render/0.19.1/source/Cargo.toml)、
[WFC manifest](https://docs.rs/crate/wfc/0.10.7/source/Cargo.toml)、
[sha2 changelog](https://docs.rs/crate/sha2/0.11.0/source/CHANGELOG.md)、
[libloading changelog](https://docs.rs/libloading/0.9.0/libloading/changelog/r0_9_0/index.html)、
[RON changelog](https://docs.rs/crate/ron/0.12.2/source/CHANGELOG.md)、
[Serde release](https://github.com/serde-rs/serde/releases/tag/v1.0.229)、
[JSON差分](https://github.com/serde-rs/json/compare/v1.0.149...v1.0.151)。

最終受入: #13は`e0da9db3`の[PR CI](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34760542442)
成功後に`874d051f`へmergeし、[merge後のCI](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34760970688)も成功した。
#16は`31835356`の[全品質ゲート](https://github.com/iaammssssstupiddddd-commits/hell-workers/actions/runs/34761079967)
成功後に`e4e1f193`へmergeし、候補とmerge commitのtreeが完全一致することを確認した。
同ゲートは通常/profiling workspace test、profiling-memory/tracy/renderdoc check、Clippy警告0件、
Python147+151 test、オンライン依存監査と全契約検査に成功。Rust testは複数構成の合計2,473回成功、失敗0。
照明の固定checksum vectorは両構成で成功し、profilingのembedded contract hashも一致した。
quality job全体は81分01秒で、Cargo cache保存まで90分上限内に収まった。

ローカルの`dev.py check`とClippy（workspace/all-targets、`-D warnings`）も成功。
ローカル`verify`は非Rust gate通過後、MemAvailableが8 GiBを下回り安全guardで停止したため、
全体結果には上記CIを用いた。rust-analyzer MCPは`Unexpected response format`で診断未取得であり、
診断0件の証拠とはしていない。実window/GPU受入は今回の検証に含めず、renderer系列変更も採用していない。

4件の処置を完了し、作業用計画は終了・削除した。新しいnative job、専用target、worktree、binary copyは作らず、
primaryの通常Cargo cacheを再利用した。storage checkは開始時・終了時とも成功し、既存2batchの管理対象容量は
4,881,321,984 bytesで不変。削除した検証job pathはなく、この作業専用の保持consumerもない。
既存consumerと通常開発cacheは維持し、採用済みのローカル作業ブランチ`review/dependabot-other-cargo`を削除した。

### 任意ツール

| ツール | 用途 | インストール |
|:---|:---|:---|
| **bacon** | ファイル変更監視 + `scripts/dev.py check` | `python3 scripts/dev.py cargo -- install bacon` |
| **cargo-expand** | Bevy derive マクロの展開確認 | `python3 scripts/dev.py cargo -- install cargo-expand` |
| **cargo-udeps** | 未使用依存クレートの検出 | `python3 scripts/dev.py cargo -- install cargo-udeps` |
| **cargo-flamegraph** | フレームグラフによるプロファイリング | `python3 scripts/dev.py cargo -- install flamegraph` |

## 便利なコマンド

### コンパイル確認
```bash
python3 scripts/dev.py check
```

### マクロ展開確認
```bash
python3 scripts/dev.py cargo -- expand --package hw_core
```

### 変更監視（自動 check）
`bacon`を使う場合も、jobをraw Cargoのcheckではなく`python3 scripts/dev.py check`に設定する。

### 画像変換
```bash
python scripts/convert_to_png.py "source_path" "assets/textures/dest.png"
```

### PNG署名確認
```bash
head -c 8 "file_path" | od -An -t x1
```

### docs インデックス更新
```bash
python3 scripts/dev.py docs --write
python3 scripts/dev.py docs --check
```

- `docs/plans/README.md` と `docs/proposals/README.md` のインデックス表を自動再生成する。
- `--check` は書き込まず、indexが古ければ失敗する（CIで実行）。
- index内容が同じ場合は更新日を書き換えない。
- 実在するファイルのみ列挙し、削除済みエントリは除去する。
- 既存エントリの Notes（手書き補足）は保持する。
- 新規ファイルはファイル内容から説明を自動抽出する。
- 計画書・提案書を追加/移動/削除したらこのコマンドを実行する。

### Visual Test Scene（TopDown建物・地形表示）
```bash
python3 scripts/dev.py cargo -- run -p visual_test
```

ゲーム本体とは独立した `visual_test` クレート。建物・地形のTopDown表示とScene RtT合成を確認する。Soulのproduction billboard / exactly-one presentation証拠にはP02/P08 native fixtureを使う。詳細は `docs/visual_test.md` を参照。

| キー / 操作 | 内容 |
|:---|:---|
| `H` | メニューパネル表示/非表示 |
| `Esc` | 終了 |
| マウス移動 | ゴーストプレビュー追従（緑=配置可 / 赤=占有）|
| 左クリック | 建築物配置 / 削除 |

ソース: `crates/visual_test/`

### 再現可能なパフォーマンス計測
```bash
python3 scripts/perf.py run \
  --workload gather --sizes medium --renders cpu --repeat 3 \
  --seed 20260712 --backend vulkan --adapter Intel \
  --window-backend wayland --present-mode novsync \
  --output target/perf-runs/gather-intel-vulkan
```

- `scripts/perf.py run` が profiling binary の計測外build、runごとの隔離、CSV/log/adapter/checksumの検証、集約を行う。比較用の計測はこの runner だけを使う。
- `--sizes`: `small`（50/4）、`medium`（200/12）、`large`（500/30）のSoul/Familiar数を選ぶ。`--souls`と`--familiars`は組で個別上書きする。
- `--renders cpu|gpu`: CPU-only寄りまたは3D RtT込みの固定描画条件を選ぶ。`--repeat 3`、backend、adapter、window backend、present modeを明示して比較する。
- binary の直接起動は起動経路のデバッグ用途だけにし、最終比較には使わない。CSV、native allocator / RSS、Tracy、RenderDocの採取条件と出力形式は[performance-profiling.md](performance-profiling.md)を正本とする。

## トラブルシューティング

### 1. Windows でのリンクエラー (too many exported symbols)
Windows の PE 形式では、一つの DLL からエクスポートできるシンボル数が 65,535 に制限されています。Bevy の `dynamic_linking` 機能を使用するとこの制限を超えやすいため、エラーが出る場合は以下の対応を行ってください。
- `Cargo.toml` の `default` features から `dynamic_linking` を削除し、静的リンクでビルドする。
- 静的リンクであってもデバッグビルドが遅い場合は、依存関係の `opt-level` を 3 に設定したままにする。

### 2. File Lock エラー
2窓で対話Cargoを使うときは、raw Cargoや通常のshellから実行せず、各ターミナルで次を
最初に実行してください。

```bash
python3 scripts/dev.py lane status
python3 scripts/dev.py lane shell
```

`lane shell` は開始時に空いている一方のlaneを取得し、shell終了まで同じlaneを保持します。
`lane status` が両方とも`busy`の場合、3つ目のsessionはcanonical `target/`へfallbackせず
終了します。既存のnative acceptance / performance runnerがcanonical `target/`を使う間は、activity lockが
busyを返し、対話Cargoのchildは起動しません。同様に対話Cargo実行中はnative/performance
recipeがexclusive取得に失敗してchildを起動しません。`--target-dir`、
`--config build.target-dir=...`、`--config build.build-dir=...`による出力先変更もwrapperで
拒否されます。lane leaseはPOSIXの`flock`を使い、未対応hostでは共有targetへfallbackせず
停止します。

### 3. Bevy ECS `error[B0001]`（Query 競合パニック）
`python3 scripts/dev.py cargo -- run` で `error[B0001]` が出る場合、同一システム内で Query のアクセス競合（例: `&mut T` と別 Query の `&T`）が発生しています。

- 原因調査: `python3 scripts/dev.py cargo -- run -p bevy_app --features bevy/dynamic_linking` で実行し、衝突した system/query 名を表示して特定する。
- 修正方針: `Without<T>` で Query を排他的に分離するか、`ParamSet` に統合して同時借用を避ける。
- 既存共通クエリ（`TaskQueries` など）がある箇所では、同種コンポーネントへの重複 Query を新設しない。

### 4. stray `target/` ディレクトリの混入
`crates/` 配下のサブディレクトリ内に `target/` が生成された場合、`cargo` をそのディレクトリから誤って実行したことが原因です。

- `**/target/` は `.gitignore` で追跡対象外に設定済みのため Git には影響しません。
- **`cargo` は必ずワークスペースルートから実行してください。**
  ```bash
  # ✅ 正しい（ワークスペースルートから）
  python3 scripts/dev.py check
  
  # ❌ 誤り（サブディレクトリ内でraw Cargoを実行すると src/ 内に target/ が生成される）
  cd crates/bevy_app/src/systems/logistics
  # direct Cargo compilation here is unsupported
  ```
- 混入した `target/` を削除するには:
  ```bash
  find crates -type d -name "target" | xargs rm -rf
  ```
- `git push` 時に `pre-push` フックが stray `target/` を自動検出して警告します。

### 5. Linux Wayland 初期化失敗（`NoCompositor`）
`Failed to build event loop: ... WaylandError(Connection(NoCompositor))` が出る場合、無効な `WAYLAND_DISPLAY` を優先してしまっている可能性があります。

- 実行時に `HW_WINDOW_BACKEND=x11` を指定すると、Wayland を無効化して X11 を強制できます。
  - 例: `HW_WINDOW_BACKEND=x11 python3 scripts/dev.py cargo -- run -p bevy_app`
- `HW_WINDOW_BACKEND=auto`（既定）では、Wayland ソケットへ接続できない場合に自動で X11 へフォールバックします。
- Wayland を明示使用する場合は `HW_WINDOW_BACKEND=wayland` を指定してください。
