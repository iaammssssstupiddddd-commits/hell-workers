# 実装・仕様整合性回復計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `implementation-spec-alignment-plan-2026-07-20` |
| ステータス | `Completed` |
| 作成日 | `2026-07-20` |
| 最終更新日 | `2026-07-21` |
| 作成者 | `Codex` |
| 関連提案 | `N/A` |
| 依存計画 | `docs/plans/archive/actionable-task-dashboard-plan-2026-07-19.md` の実装commit `fdf045d5`をbaselineとしてM0完了 |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題:
  - 現行実装と仕様文書の比較で、実装バグ、意図的な実装変更に追従していない文書、未登録の重複system、
    crate境界をまたぐ二重入力経路が混在している。
  - すべてを古い仕様へ戻すと、Dream獲得量の不整合は残る一方、空間indexを使う建設・ドア処理や
    専用通知へ分離したタスク中断処理を退行させる。
  - 並行中のactionable task dashboard作業がMessage、UI intent、AI ordering、crate README、durable docsを
    広く変更しているため、古いbaselineのまま本計画を実装すると正当な並行差分を上書きする。
- 到達したい状態:
  - `DreamPool`へ実際に移送した量と、Dream獲得presentationが保持・表示する質量が一致する。
  - Familiarの新規recruit閾値とrelease閾値の役割を分離し、設定値`0.0`を含む全範囲で挙動が定義される。
  - constructionとdoorはindex利用挙動を維持しつつ、実装本体を責務に合うLeaf crateへ置き、rootを登録・配線へ戻す。
  - Zone配置とUI actionは到達可能なstateと単一のintent consumerだけを持つ。
  - AI phase、task lifecycle、crate境界、イベント台帳、UI・Visual仕様が実際のproduction登録経路と一致する。
- 成功指標:
  - sleeping・RestAreaの各producerについて、同じdriver実行内の`DreamPool.points`増分とtransfer Message合計が
    絶対誤差`1e-5`以内で一致する。
  - UI root/cameraがないframe、RestArea退出と同frameの最終drain、world replacementでDream massをsilentに失わない。
  - release閾値が`f32::EPSILON`より大きければ`recruit < release`となり、
    release`0.0`ではrecruitを無効化する。
  - construction phase transitionとdoor proximityは、各用途でindex利用版のproduction実装が一つだけ存在する。
  - `MovePlantBuilding`、`ToggleDoorLock`、`SelectArchitectCategory`は、押下ごとに一つの`UiIntent` consumerだけが
    副作用を実行する。
  - 差異監査表の全IDについて、コード正本、更新文書、確認方法、完了状態を追跡できる。

## 2. スコープ

### 対象（In Scope）

- 並行差分を確定した後のbaseline再監査:
  - actionable task dashboard作業と重なるファイル・契約をM0で再確認し、実装済み項目、残存差異、所有者を確定する。
- 実装を仕様・不変条件へ合わせる修正:
  - Dreamの実移送量をproduction slow-simulation driverからpresentationへ渡し、獲得UIの質量保存を成立させる。
  - Familiarのrecruit/release疲労閾値へ明示的なヒステリシスと`0.0`の無効値契約を導入する。
- 現行の最適化挙動を維持した構造整理:
  - `TileSiteIndex`利用版construction処理を`hw_logistics`へ、`SpatialGrid`利用版door処理を`hw_spatial`へ置き、
    rootはproduction ordering facadeとして登録する。
  - 到達不能な`PlayMode::ZonePlace`を除去し、Zone配置を
    `TaskDesignation + TaskMode::ZonePlacement`へ統一する。
  - `hw_ui`からBuildingドメイン判定をroot adapterへ移す。
  - direct `Changed<Interaction>` consumerを廃止し、対象MenuActionの副作用をintent handlerへ統合する。
- 現行実装を正とする文書同期:
  - task lifecycle、AI phase、Dream/World/UI/Visual、イベント、workspace、crate境界、state、性能説明。
- 変更した契約のunit/integration/system-orderテスト、手動シナリオ、workspace全体の品質ゲート。

### 非対象（Out of Scope）

- actionable task dashboard自体の診断、priority、cancel、notification、save schema実装。
  - 本計画はM0でその完成状態へ追従するが、同計画の成果を再実装・revertしない。
- Motivation低下による即時タスク放棄の実装。
  - 導入する場合は新規割当拒否、閾値継続時間、再受付cooldown、retryable abortを別計画にする。
- DreamQuality別の放出レート復活。`DreamQuality`は引き続きvisual-onlyとする。
- RestAreaのoccupant数で生成するambient world粒子をDream transfer massへ変換すること。
- `TileSiteIndex`自体の`hw_jobs`移設、`SpatialGrid`自体の`hw_world`移設など、index型の所有権変更。
- door proximityへFamiliarを追加すること。現行契約どおりSoulだけを対象にする。
- `FloorPlace`の`ConstructionAreaPlace`等への名称変更。
- 新しいWorkType、TransportRequest、建築物、バランス機能の追加。
- 定数値の包括的な再調整、ログ基盤刷新、イベントカタログ自動生成ツールの新設。

## 3. 現状とギャップ

### 3.1 判断区分

| 区分 | 対象 | 本計画の判断 |
| --- | --- | --- |
| 仕様・不変条件優先 | Dream獲得mass | 後段の再計算を廃止し、producerが実際に移送したdeltaを正本にする |
| 仕様再定義後に実装 | Familiar fatigue | release`0.0`をrecruit無効値とし、正の設定では厳密なヒステリシスを保証する |
| 現行挙動＋境界是正 | construction / door | index利用挙動を維持し、実装本体はLeaf、登録と全体orderingはrootにする |
| 現行挙動＋整理 | AI phase / `ApplyDeferred` | 必要なmutationとbarrierを維持し、文書を「原則＋明示的例外」に直す |
| 現行挙動＋整理 | Zone / UI action | 到達不能stateとdirect Interaction consumerを除去し、本番経路を一意にする |
| 現行実装優先 | task、DreamQuality、World、workspace | 意図的な移行・最適化を戻さず、差異ID単位で文書と台帳を同期する |

### 3.2 Dream質量とproduction経路

- productionで登録されているのは単独の`dream_update_system` / `rest_area_update_system`ではなく、
  `slow_simulation_driver_system`である。
- driverは一frameに最大5 slow stepを実行し、各step内で`rest_area_update_step`の後に
  `dream_update_step`を直接呼ぶ。
- sleeping側はLogic後の`Soul.dream`から別式で表示量を再計算するため、残量が0になる最終stepを過少計上する。
- RestArea獲得UI側はoccupant数と描画intervalからmassを作り、実際のdrain量と対応していない。
- RestArea最終drainでは同frameのExecuteで`RestingIn`が除去されるため、Visualが後からrelationshipをQueryしても
  source anchorを復元できない。
- 現Visualはcamera/UI root不在時に早期returnするため、transient Messageをその場で描画するだけではmassを失う。

### 3.3 Familiar疲労閾値

- `FamiliarOperation::fatigue_threshold`はrecruit、release、既存memberのtask assignmentへ同じ値で渡される。
- `max(0, release - 0.2)`だけではrelease`0.0`時にrecruitも`0.0`となり、`recruit < release`を満たせない。
- 現settings UIは`0.0..=1.0`を許容するため、`0.0`の意味を計画内で確定する必要がある。
- scouting/recruitmentだけでなく、state decisionから渡される候補specもrecruit閾値へ統一する必要がある。
- 既存memberのtask assignmentはrecruit条件ではないため、release側の保存値を引き続き使う。

### 3.4 production実装とcrate所有境界

- constructionはrootの`TileSiteIndex`利用版が登録され、`hw_jobs`の全走査版は公開されているが未登録である。
- doorはrootの`SpatialGrid`利用版が登録され、`hw_world`の全走査版は公開されているが未登録である。
- index利用は維持すべきだが、両root systemはroot-only型を必要とせず、domain判定とworld mutationを直接所有する。
- `docs/crate-boundaries.md`はroot-only依存がない実装本体をLeafへ置き、必要ならrootをordering facadeにする契約である。
- 現依存グラフでは`hw_logistics`が`TileSiteIndex`・`hw_jobs`を参照でき、
  `hw_spatial`が`SpatialGrid`・`hw_world`・`hw_jobs`を参照できる。

### 3.5 Zone、UI action、並行作業

- Zone選択は`PlayMode::TaskDesignation`へ遷移する一方、`PlayMode::ZonePlace`がenumと表示分岐に残る。
- `hw_ui::hover_action_button_system`が`Building`を直接Queryし、presentation crateがゲームドメイン判定を持つ。
- MenuActionは`UiIntent`を書いた後、generic handlerが一部variantをno-opで読み、専用systemが
  `Changed<Interaction>`を再読して副作用を実行する。
- direct consumerは`ForegroundUiGate`と`ResolvedInputFrame::pointer_selection_suppressed()`を持つため、
  intent移行時にproducer gateとconsumer validationを分けて維持する必要がある。
- actionable task dashboard作業は`messages.rs`、`intent_handler.rs`、`core.rs`、`hw_ui/intents.rs`、
  AI ordering、`hw_jobs/src/lib.rs`、多数のdurable docsを変更中であり、本計画の全code milestoneと重なる。

### 3.6 文書の陳腐化

- `Holding/HeldBy`、Motivation即時放棄、固定Stress増加率、NightTerrorの放出量0など、現行実装に反する説明が残る。
- AI phaseのread-only規則と`ApplyDeferred`説明が、取消consumerやAutoGatherの同frame確定例外を表せていない。
- event/message、TransportRequest、WorkType、workspace crate、依存辺、型の正本、移設済みpathの台帳が不足する。
- Room境界表示、RestArea退出位置、InfoPanel、debug Gizmos、power schedule、speech scan、visual test cameraなど、
  現在の表示・実行順を古い形で説明する箇所がある。

## 4. 実装方針（高レベル）

### 4.1 baselineと所有権

- M0完了前にM1以降へ着手しない。
- M0では並行計画の最新diffを読み、重複ファイルごとに「本計画で変更」「並行計画で完了済み」
  「契約再評価が必要」を記録する。正当な並行差分を上書き・revertしない。
- Domainの数値変化は、変更を行ったproducerが算出したdeltaを正本とする。
- index利用という性能契約と、実装ファイルの現配置を別判断にする。index利用は維持し、root-only依存がない
  domain実装はLeafへ移し、rootは登録・cross-crate orderingだけを所有する。
- 文書は型・定数の転記より、所有者、writer/consumer、ordering、例外、silent failure条件を記録する。

### 4.2 Dream transfer契約

```text
slow_simulation_driver_system
  ├─ frame accumulatorをclear
  ├─ slow stepごとにrest/dreamのactual_drainをSoul単位で加算
  └─ 全step終了後、Soulごとに最大1 Messageを発行
                                  │ Logic → Visual
                                  v
always-on ingestion ──> durable pending ledger
                                  │ camera/UI/anchor準備済み
                                  v
popup / 獲得UI particle
```

- production ownerである`slow_simulation_driver_system`がframe accumulatorと
  `MessageWriter<DreamTransferredVisualMessage>`を所有する。
- `rest_area_update_step` / `dream_update_step`は、`DreamPool`へ加算した同じ`actual_drain`を
  accumulatorへ渡す。driver loop後に`amount > 0`のSoulだけを一件発行する。
- accumulatorは最初の正のdrainでsource/quality snapshotを固定し、同じdriver実行内の後続drainが
  同じsource/qualityであることをtestする。異なる属性を最後の値で黙って上書きしない。
- Messageは最低限、次をdrain時点のsnapshotとして持つ。
  - `soul: Entity`
  - `amount: f32`
  - `quality: DreamQuality`
  - `source: Sleeping { origin: Vec2 } | RestArea { rest_area: Entity, origin: Vec2 }`
  - `is_final: bool`（producerが同じstepでtransfer streamの終了を確定した場合だけ`true`）
- RestArea sourceは`RestingIn`をproducer Queryで取得し、退出前のrest area Entityとfallback world位置を保存する。
  Visualは後段で`RestingIn`を再Queryしない。
- fallback originはrest areaの`Transform`を優先し、producer時点でrest areaを解決できない場合だけSoulの
  `Transform`を使う。
- sleepingはdrain時点の`DreamState.quality`、RestAreaは既存のVivid presentation契約をMessageへ保存する。
- ingestion systemはcamera、UI root、source EntityをQueryする前に毎frame Messageを全件読み、
  `hw_visual`所有のdurable pending ledgerへ加算する。
- camera/UI root不在、source despawn、projection失敗、particle cap到達時はpendingを保持する。
  source Entityが消失した場合はMessageに保存した`origin`を使う。
- world replacementではMessage bufferとpending ledgerを同じreset境界でclearする。
- `amount`は有限かつ正であることをproducer/ingestion境界で検証する。
- popupと獲得UI particleは同じtransferの二つの表現であり、両者を加算してPool増分と比較しない。
  ledgerはchannel別pendingまたはdelivery flagを持ち、各presentation channelへの引き渡しを個別に記録する。
  一方の表示成功で他方のpendingを消さず、二つのchannelをPool massとして加算もしない。
- RestAreaのambient world粒子はoccupant数ベースの演出として維持し、獲得UI mass ledgerへ含めない。
- NightTerrorを含む`DreamQuality`は表示属性にだけ影響し、transfer amountには影響させない。

### 4.3 Familiar hysteresis契約

```text
release = FamiliarOperation.fatigue_threshold

if release <= f32::EPSILON:
    recruit = Disabled
else:
    recruit = Some(max(0, release - FAMILIAR_RECRUIT_FATIGUE_HYSTERESIS))
```

- 共通APIは`Option<f32>`等で「recruit無効」と数値閾値を区別する。
- release`0.0`は「このFamiliarは新規Soulをrecruitしない」という明示的な設定値とする。
  save由来の`f32::EPSILON`以下の微小値も安全側で無効として扱う。
- releaseが`f32::EPSILON`より大きければ、導出したrecruit閾値は必ずreleaseより厳密に低い。
- scouting、recruitment、候補score、候補再検証、state decisionのspec生成は同じ共通APIを使う。
- 既存Squadメンバーのreleaseとtask assignmentは保存中のrelease閾値を使い、recruit閾値へ狭めない。
- settings UIとsave schemaの公開フィールド・範囲は増やさず、`0.0`と表示値の意味を文書化する。
- 境界fixtureは`0.0`、`0.1`、`0.2`、`0.8`、`1.0`を含める。

### 4.4 construction / doorの所有契約

- construction:
  - `TileSiteIndex`を使うfloor/wall phase transition実装を`hw_logistics`へ移す。
  - `hw_jobs`はconstruction state/type、phase eligibility/state transitionのpure helper、cleanup helperを所有する。
  - `hw_logistics`のindex-backed adapterは対象Entityを絞り込み、`hw_jobs`のpure helperに判定を委譲する。
  - rootは必要なorderingを保つため、`hw_logistics`が公開するsystemを一箇所だけ登録する。
  - root版と`hw_jobs`全走査版を削除し、0 tile・index不一致・partial mutation防止を維持する。
- door:
  - `SpatialGrid`を使うnearby door system実装を`hw_spatial`へ移す。
  - `hw_world`はdoor proximity/state decisionのpure helper、door state適用、WorldMap mutation、visual handlesを所有する。
  - `hw_spatial`のindex-backed adapterは近傍候補を供給し、`hw_world`のpure helperに判定を委譲する。
  - rootは`hw_spatial`が公開するsystemを一箇所だけ登録する。
  - root版と`hw_world`全走査版を削除し、lock、close timer、Soul-only対象を維持する。
- M0時点の依存グラフが上記配置を不可能に変えていた場合、rootにdomain実装を黙って残さず、
  計画を一度`Blocked`にして所有先を再設計する。

### 4.5 AI phase、Zone、UI action契約

- AI phase:
  - user取消をExecute前に反映するconsumerと、AutoGather後の同frame委譲を成立させるbarrierを維持する。
  - blanketな「全フェーズ間barrier」ではなく、Commands producerと直後consumerの組ごとに必要性を記録する。
- Zone:
  - `PlayMode::ZonePlace`を除去し、表示・入力・save/reset testを
    `TaskDesignation + TaskMode::ZonePlacement`へ合わせる。
- UI:
  - root adapterがhover中EntityのPlant building適格性を解決し、`hw_ui`所有のdomain-neutral target/ViewModelへ渡す。
  - `hw_ui`はscreen projection、hover latch、overlay描画だけを担当する。
  - `ui_interaction_system`はbutton Entityを持つproducerとして`ForegroundUiGate`を適用してから`UiIntent`を発行する。
  - `MovePlantBuilding`、`ToggleDoorLock`、`SelectArchitectCategory`はgeneric handlerから専用handlerへ委譲し、
    direct `Changed<Interaction>` consumerを削除する。
  - Architect categoryの同一category再押下によるtoggleと、door lockの即時WorldMap/visual反映を維持する。
  - Move consumerは`pointer_selection_suppressed()`を維持し、対象Entityの存在とPlant categoryをapply時に再検証する。
  - Door consumerも対象Entityの存在とDoor componentをapply時に再検証する。
  - stale/invalid intentはdrainして副作用なしとし、unpause/overlay解除後へ遅延適用しない。
  - Moveの成功順序は`active mode cleanup → selection/move context → mode遷移 → menu visibility`で固定する。

### 4.6 Bevy 0.19 APIでの注意点

- Messageの同一`Update`内可視性はproducerをLogic、ingestionをVisual先頭へ順序付けし、
  production pluginを通すsystem-order testで確認する。
- `Commands`を使うAI処理では`ApplyDeferred`を推測で変更せず、同frame component変化を必要とする
  producer/consumer pairだけに置く。
- Message consumerは表示依存resource/queryを取得できない場合でもMessageをdrainできる構成にする。
- 新規Queryが複雑になる場合はtype aliasまたは`SystemParam`で整理し、Clippy allowを追加しない。

## 5. マイルストーン

> M0は全code milestoneの前提条件である。M1～M8はM0完了まで開始しない。

## M0: 並行作業の確定とbaseline再監査

- 変更内容:
  - actionable task dashboard作業の完了状態とworktree所有者を確認する。
  - 本計画と重なる全ファイルについて最新diffを読み、残存差異と既に解消された差異を再分類する。
  - production registration、crate依存、Message inventory/reset、UI intent consumer、AI orderingを再確認する。
  - 再監査結果で候補ファイル・テスト名・差異監査表を更新し、ステータスを`In Progress`へ変更してからM1へ進む。
- 直接編集するファイル:
  - `docs/plans/implementation-spec-alignment-plan-2026-07-20.md`
- 完了条件:
  - [x] actionable task dashboardの進行中差分がcommit済み、またはファイル所有者と統合順序が明示されている
  - [x] `git status --short`の各重複ファイルについて本計画が上書きしない根拠が記録されている
  - [x] M1～M8の候補ファイルとproduction経路が最新コードへ再照合されている
  - [x] M0完了前にcode/docs実装milestoneへ着手していない
- 検証:
  - `git diff --check`
  - `python3 scripts/dev.py docs --check`
- ロールバック境界:
  - 計画書だけのbaseline更新として単独で戻せる。

## M1: Dream実移送量と獲得presentation massの統一

- 変更内容:
  - `hw_core`にsource snapshot付きDream transfer Messageを定義する。
  - `slow_simulation_driver_system`にSoul単位frame accumulatorとloop後Message発行を追加する。
  - sleeping/RestAreaのstep関数は実際のdeltaだけをaccumulatorへ渡す。
  - producerがstream終了を確定したtransferには`is_final`を付け、Visualはslow-step間の無Message frameを
    終了扱いせず、明示的finalまたは0.5秒の連続無通信で最終tailをflushする。
  - `hw_visual`にalways-on ingestionとdurable pending ledgerを追加する。
  - sleeping popupとRestArea獲得UI粒子をledger駆動へ変更し、stateからの再計算・固定mass生成を廃止する。
  - ambient RestArea world粒子は変更しない。
  - Message登録、world replacement reset、mass保存、最終drain、catch-up、anchor fallbackをテストする。
- 変更候補ファイル:
  - `crates/hw_core/src/events.rs`
  - `crates/bevy_app/src/plugins/messages.rs`
  - `crates/hw_visual/src/lib.rs`の既登録world-replacement hook（root reset本体は変更不要）
  - `crates/hw_soul_ai/src/soul_ai/update/slow_simulation.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/dream_update.rs`
  - `crates/hw_soul_ai/src/soul_ai/update/rest_area_update.rs`
  - `crates/hw_visual/src/dream/components.rs`
  - `crates/hw_visual/src/dream/gain_visual.rs`
  - `crates/hw_visual/src/dream/particle.rs`
  - `crates/hw_visual/src/lib.rs`
  - `crates/bevy_app/src/plugins/messages/tests/mod.rs`
  - `docs/dream.md`
  - `docs/dream-visual.md`
  - `docs/events.md`
- 必須テスト名:
  - `slow_simulation_emits_one_dream_transfer_per_soul_per_frame`
  - `sleeping_final_drain_preserves_transfer_mass`
  - `rest_area_final_drain_keeps_captured_anchor_after_exit`
  - `dream_transfer_ingestion_preserves_mass_without_ui_or_camera`
  - `slow_step_gap_does_not_flush_pending_presentation`
  - `sleeping_partial_drain_is_not_marked_final`
  - `dream_pending_ledger_clears_on_world_replacement`
  - `dream_quality_does_not_change_transfer_amount`
  - `messages_plugin_registers_dream_transfer_message`
- 完了条件:
  - [x] driver一回についてPool増分とMessage合計が絶対誤差`1e-5`以内で一致する
  - [x] 同じSoulからdriver一回あたり最大一件だけMessageが発行される
  - [x] RestArea退出と同frameの最終drainがcaptured anchor/fallback origin付きでledgerへ入る
  - [x] UI/camera/source不在、particle cap、描画intervalでpending massを失わない
  - [x] slow-step間の通常frameをstream終了と誤認せず、明示的finalまたは0.5秒の無通信だけでtailをflushする
  - [x] popupとUI particleを二重加算せず、各channelのledger debitを追跡できる
  - [x] world replacementでMessageとpending ledgerが空になる
  - [x] DreamQualityはtransfer rateに影響せず、ambient RestArea粒子契約も維持する
- 検証:
  - `cargo test --locked -p hw_soul_ai --lib`
  - `cargo test --locked -p hw_visual --lib`
  - `cargo test --locked -p bevy_app --lib`
  - `cargo test --locked -p bevy_app --lib messages_plugin_registers_dream_transfer_message`
  - `cargo test --locked -p bevy_app --lib dream_transfer -- --nocapture`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p hw_soul_ai -p hw_visual -p bevy_app --all-targets -- -D warnings`

## M2: Familiar recruit/releaseヒステリシス

- 変更内容:
  - release閾値から`Option`のrecruit閾値を導く共通APIを追加する。
  - release`0.0`をrecruit無効値として扱う。
  - scouting/recruitmentの候補判定・score・再検証・state decision specを共通APIへ統一する。
  - releaseと既存member assignmentは保存中のrelease閾値を使う。
  - UI表示、`0.0`の説明、不変条件をdurable docsへ反映する。
- 変更候補ファイル:
  - `crates/hw_core/src/familiar.rs`
  - `crates/hw_core/src/constants/ai.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/scouting.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/recruitment.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/state_decision/system.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/squad.rs`
  - `crates/hw_familiar_ai/src/familiar_ai/decide/helpers.rs`
  - `crates/bevy_app/src/interface/ui/interaction/systems.rs`
  - `crates/bevy_app/src/interface/ui/presentation/builders.rs`
  - `docs/familiar_ai.md`
  - `docs/invariants.md`
- 必須テスト名:
  - `zero_release_threshold_disables_recruitment`
  - `positive_release_threshold_has_strictly_lower_recruit_threshold`
  - `recruit_threshold_boundaries_are_defined`
  - `recruit_and_release_use_distinct_thresholds`
  - `member_task_assignment_keeps_release_threshold`
- 完了条件:
  - [x] release`0.0`では候補score・validation・recruitが全て無効になる
  - [x] releaseが`f32::EPSILON`より大きい全設定範囲で`recruit < release`となる
  - [x] release`0.8`に対してrecruit`0.6`となる
  - [x] `0.1`、`0.2`、`1.0`の境界が共通helperと全consumerで一致する
  - [x] recruit可能なSoulが同じ疲労値のまま直後にreleaseされない
  - [x] settings/saveの公開フィールドと`0.0..=1.0`の範囲を維持する
  - [x] 既存memberのtask assignmentをrecruit閾値で誤って狭めない
- 検証:
  - `cargo test --locked -p hw_core --lib`
  - `cargo test --locked -p hw_familiar_ai --lib`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p hw_core -p hw_familiar_ai --all-targets -- -D warnings`

## M3: index利用construction実装のLeaf所有化

- 変更内容:
  - rootのindex利用floor/wall transitionを`hw_logistics`へ移す。
  - `hw_jobs`の未登録全走査systemとre-export、移設後のroot実装を除去する。
  - root ordering facadeからLeaf実装を一度だけ登録する。
  - 0 tile、index不一致、別site隔離、partial mutation防止、対象tile数counterを固定する。
  - architecture/cargo workspace/building docsを同じmilestoneで更新する。
- 変更候補ファイル:
  - `crates/hw_logistics/src/construction_phase_transition.rs`（新規候補）
  - `crates/hw_logistics/src/lib.rs`
  - `crates/hw_jobs/src/construction.rs`
  - `crates/hw_jobs/src/lib.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/phase_transition.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/phase_transition.rs`
  - `crates/bevy_app/src/systems/jobs/construction_metrics.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/output.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/capture_driver.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - root production registration箇所
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/building.md`
- 必須テスト名:
  - `indexed_floor_transition_inspects_only_site_tiles`
  - `indexed_wall_transition_inspects_only_site_tiles`
  - `zero_tile_site_does_not_transition`
  - `index_mismatch_does_not_partially_transition`
  - `construction_transition_has_single_production_registration`
- 完了条件:
  - [x] floor/wallごとにindex利用production実装が一つだけ存在する
  - [x] 実装本体はLeaf、rootは登録・orderingだけを持つ
  - [x] 0 tileとindex不一致siteが遷移しない
  - [x] 別siteのtileを走査・mutationしない
  - [x] profiling counterで全world scanを再導入していないことを自動検証する
- 検証:
  - `cargo test --locked -p hw_logistics --lib construction_phase`
  - `cargo test --locked -p bevy_app --lib construction_phase`
  - `cargo test --locked -p bevy_app --lib --features profiling construction_transition`
  - `cargo test --locked -p hw_logistics --lib`
  - `cargo test --locked -p bevy_app --lib`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p hw_logistics -p bevy_app --all-targets -- -D warnings`

## M4: index利用door実装のLeaf所有化

- 変更内容:
  - rootの`SpatialGrid`利用door proximity実装を`hw_spatial`へ移す。
  - `hw_world`の未登録全走査systemとre-export、移設後のroot実装を除去する。
  - `apply_door_state`、WorldMap mutation、visual handlesは`hw_world`に維持する。
  - root ordering facadeからLeaf実装を一度だけ登録する。
  - Soul-only近接、path上door判定、lock、close timer、index候補counterを固定する。
  - architecture/cargo workspace/building docsを同じmilestoneで更新する。
- 変更候補ファイル:
  - `crates/hw_spatial/src/door_proximity.rs`（新規候補）
  - `crates/hw_spatial/src/lib.rs`
  - `crates/hw_world/src/door_systems.rs`
  - `crates/hw_world/src/lib.rs`
  - `crates/bevy_app/src/systems/jobs/door_proximity.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/output.rs`
  - `crates/bevy_app/src/plugins/startup/perf_scenario/capture_driver.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - root production registration箇所
  - `docs/architecture.md`
  - `docs/cargo_workspace.md`
  - `docs/building.md`
- 必須テスト名:
  - `door_proximity_considers_souls_only`
  - `soul_path_to_door_opens_unlocked_door`
  - `locked_door_does_not_auto_open`
  - `door_close_timer_resets_for_nearby_soul`
  - `indexed_door_query_does_not_scan_all_souls`
  - `door_proximity_has_single_production_registration`
- 完了条件:
  - [x] index利用door production実装が一つだけ存在する
  - [x] 実装本体はLeaf、rootは登録・orderingだけを持つ
  - [x] unlocked doorが近接Soulで開閉し、Familiarだけでは開かない
  - [x] locked doorとclose timerの挙動を維持する
  - [x] profiling counterで全Soul scanを再導入していないことを自動検証する
- 検証:
  - `cargo test --locked -p hw_spatial --lib door_proximity`
  - `cargo test --locked -p bevy_app --lib door_proximity`
  - `cargo test --locked -p bevy_app --lib --features profiling door_proximity`
  - `cargo test --locked -p hw_spatial --lib`
  - `cargo test --locked -p bevy_app --lib`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p hw_spatial -p hw_world -p bevy_app --all-targets -- -D warnings`

## M5: AI ordering契約の固定

- 変更内容:
  - user取消がExecute前に反映されるproduction順序をintegration testで固定する。
  - AutoGatherの`Commands`がflushされた後、task revision sync、delegationの順で同frameに観測される契約を
    actionable task dashboard完了後の最新set名へ合わせる。
  - 既存barrierの一律削除・追加は行わず、producer/consumer pair単位で必要性を文書化する。
- 変更候補ファイル:
  - M0で確定したroot/Leaf plugin registrationとtest support
  - `docs/ai-system-phases.md`
  - `docs/architecture.md`
- 必須テスト名:
  - `user_cancellation_is_visible_before_execute`
  - `auto_gather_flush_is_visible_to_same_frame_delegation`
  - `ai_barriers_match_documented_producer_consumer_pairs`
- 完了条件:
  - [x] user取消済みtaskを同frameのExecuteが開始しない
  - [x] AutoGather指定を同frameのdelegationが参照できる
  - [x] barrierの根拠がproducer、consumer、必要component mutationと対応する
  - [x] actionable task dashboardのdiagnostics orderingを退行させない
- 検証:
  - `cargo test --locked -p bevy_app --lib user_cancellation_is_visible_before_execute`
  - `cargo test --locked -p bevy_app --lib auto_gather_flush_is_visible_to_same_frame_delegation`
  - `cargo test --locked -p bevy_app --lib familiar_ai`
  - `cargo test --locked -p bevy_app --lib`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p bevy_app --all-targets -- -D warnings`

## M6: 到達不能なZone stateの除去

- 変更内容:
  - `PlayMode::ZonePlace`と対応する表示分岐を削除する。
  - Zone開始、drag、確定、Esc取消、save/resetの期待値を
    `TaskDesignation + TaskMode::ZonePlacement`へ統一する。
  - state docsを同じmilestoneで更新する。
- 変更候補ファイル:
  - `crates/hw_core/src/game_state.rs`
  - `crates/bevy_app/src/interface/ui/interaction/mode.rs`
  - Zone input/save/reset test
  - `docs/state.md`
- 必須テスト名:
  - `zone_placement_flow_does_not_require_zone_place_state`
  - `escape_cancels_zone_placement_to_normal`
  - `world_replace_clears_zone_placement_mode`
- 完了条件:
  - [x] production codeとcurrent durable docsに`PlayMode::ZonePlace`が残らない
  - [x] Zone配置の開始・drag・確定・Esc取消が既存経路で動く
  - [x] save/load/world replacement後に到達不能stateを復元しない
- 検証:
  - `cargo test --locked -p hw_core --lib`
  - `cargo test --locked -p hw_core --lib game_state`
  - `cargo test --locked -p bevy_app --lib zone`
  - `cargo test --locked -p bevy_app --lib systems::save`
  - `cargo test --locked -p bevy_app --lib`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p hw_core -p bevy_app --all-targets -- -D warnings`

## M7: UI domain boundaryとMenuActionの単一intent化

- 変更内容:
  - Plant building判定をroot adapterへ移し、`hw_ui`へdomain-neutral hover target/ViewModelを渡す。
  - `update_hover_entity → root domain adapter → widget sync`のsystem orderを明示する。
  - `MovePlantBuilding`、`ToggleDoorLock`、`SelectArchitectCategory`をintent handler配下へ移す。
  - direct `Changed<Interaction>` consumerを削除する。
  - producer gate、pointer suppression、domain再検証、cleanup/menu ordering、world replacementをテストする。
- 変更候補ファイル:
  - `crates/hw_ui/src/interaction/hover_action.rs`
  - `crates/hw_ui/src/intents.rs`
  - `crates/hw_ui/src/lib.rs`
  - `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`
  - `crates/bevy_app/src/interface/ui/interaction/handlers/`
  - `crates/bevy_app/src/interface/ui/interaction/systems.rs`
  - `crates/bevy_app/src/interface/ui/plugins/core.rs`
  - `docs/cargo_workspace.md`
  - `docs/crate-boundaries.md`
  - `crates/hw_ui/README.md`
- 必須テスト名:
  - `move_overlay_is_limited_to_plant_buildings`
  - `move_plant_intent_is_consumed_once`
  - `move_plant_intent_rejects_despawned_or_non_plant_target`
  - `pointer_suppression_blocks_move_plant_intent`
  - `foreground_gate_blocks_background_menu_action`
  - `door_and_architect_actions_have_single_intent_consumer`
  - `move_action_cleanup_precedes_mode_and_menu_update`
- 完了条件:
  - [x] `hw_ui`が`Building` / `BuildingCategory`をQueryしてhover対象を決めない
  - [x] Plant buildingだけにMove overlayが表示され、button hover中のtarget保持を維持する
  - [x] hover entity更新、domain判定、widget同期が同frameで決定的な順序を持つ
  - [x] 対象3 MenuActionにdirect Interaction副作用consumerが残らない
  - [x] stale/despawn/non-domain targetをapply時に安全に拒否する
  - [x] modal/pause/pointer suppression/foreground gateで背景操作を通さない
  - [x] cleanup、context更新、mode遷移、menu visibilityの順序がtestで固定される
- 検証:
  - `cargo test --locked -p hw_ui --lib`
  - `cargo test --locked -p bevy_app --lib move_plant`
  - `cargo test --locked -p bevy_app --lib menu_action`
  - `cargo test --locked -p bevy_app --lib`
  - `cargo check --locked --workspace`
  - `cargo clippy --locked -p hw_ui -p bevy_app --all-targets -- -D warnings`

## M8: 差異ID単位の仕様・台帳監査

- 方針:
  - M1～M7で変更した契約文書は各milestoneで同期する。
  - M8は残存docs-only差異の修正と、全IDの横断監査に限定する。
  - 各IDを`Pending → Verified`へ更新し、コード正本と確認結果を記録する。

| ID | 維持する現行契約 | コード正本 | Owner | 主な文書 | 状態 |
| --- | --- | --- | --- | --- | --- |
| D01 | `Inventory(Option<Entity>)`、現在の`CommandedBy` writer/remover、疲労・ストレス時`emit_abandoned=false` | `hw_logistics/src/types.rs`、`hw_soul_ai/src/soul_ai/helpers/work.rs`、`bevy_app/src/entities/damned_soul/observers.rs` | M8 | `tasks.md`、`invariants.md`、`soul_ai.md` | Verified |
| D02 | 低Motivationは速度・idle判断へ影響するがactive taskを即時放棄しない | `hw_soul_ai/src/soul_ai/update/`、`hw_soul_ai/src/soul_ai/decide/` | M8 | `soul_ai.md` | Verified |
| D03 | `ReturnWheelbarrow`とSoulSpa transport producerはactive requestである | `hw_logistics/src/transport_request/producer/wheelbarrow.rs`、`bevy_app/src/systems/jobs/soul_spa_construction/auto_haul.rs` | M8 | `logistics.md`、`tasks.md`、`DEVELOPMENT.md` | Verified |
| D04 | Dreamは同率drain、quality visual-only、実deltaが獲得massの正本 | `hw_soul_ai/src/soul_ai/update/slow_simulation.rs`、`hw_visual/src/dream/gain_visual.rs` | M1 | `dream.md`、`dream-visual.md`、`events.md` | Verified |
| D05 | Roomは境界線表示、RestArea退出は保持中の隣接位置で再表示する | `hw_world/src/room_detection/`、`hw_soul_ai/src/soul_ai/update/rest_area_update.rs` | M8 | `room_detection.md`、`rest_area_system.md` | Verified |
| D06 | 現行WorkType、Energy Demand/Grid、Dream stat、theme typographyを正とする | `hw_core/src/jobs.rs`、current UI ViewModel/widget、`hw_ui/src/theme.rs` | M8 | `task_list_ui.md`、`soul_energy.md`、`info_panel_ui.md`、`fonts.md` | Verified |
| D07 | task linkはdebug Gizmos、visual testは現行camera構成 | `bevy_app/src/systems/debug/`、`bevy_app/src/bin/visual_test/` | M8 | `building.md`、`debug-features.md`、`visual_test.md` | Verified |
| D08 | SoulSpa power ordering、speech shard、通常時INFO抑制を正とする | `bevy_app/src/systems/energy/`、`hw_visual/src/speech/`、gathering production systems | M8 | `building.md`、`architecture.md`、`speech_system.md`、`gathering.md` | Verified |
| D09 | Perceive/Decide mutationとtargeted `ApplyDeferred`を明示例外とする | `bevy_app/src/plugins/logic.rs`、production AI set helpers | M5 | `ai-system-phases.md`、`architecture.md` | Verified |
| D10 | event/message、workspace依存、`AssignedTask`/`TaskMode`正本、移設pathを追跡する | `bevy_app/src/plugins/messages.rs`、`Cargo.toml`群、`hw_jobs/src/tasks/mod.rs`、`hw_core/src/game_state.rs` | M1～M8 | `events.md`、`cargo_workspace.md`、`state.md`、`DEVELOPMENT.md`、`README.md` | Verified |
| D11 | `SpatialGridOps`契約と`SpatialGrid`実装、index利用system所有を現配置へ合わせる | `hw_world/src/spatial.rs`、`hw_spatial/src/grid.rs`、`hw_logistics/src/construction_phase_transition.rs`、`hw_spatial/src/door_proximity.rs` | M3/M4 | `architecture.md`、`cargo_workspace.md`、`crate-boundaries.md` | Verified |
| D12 | Stressは基礎増加・Dream係数・監視加算の合成である | `hw_soul_ai/src/soul_ai/update/vitals_update.rs`、`hw_soul_ai/src/soul_ai/update/vitals_influence.rs` | M8 | `soul_ai.md` | Verified |

- 完了条件:
  - [x] D01～D12が全て`Verified`で、各行に確認したコード正本と更新文書がある
  - [x] 削除済み型・未登録system・不存在WorkTypeを現行契約として記載しない
  - [x] event/messageとTransportRequestの登録・producer/consumer/timingが追跡できる
  - [x] workspace crateと主要依存辺がcurrent `Cargo.toml`群と一致する
  - [x] milestone文書をM8で重複改稿せず、差異Ownerへ戻して同じ変更単位で直す
  - [x] stale pathを戻すためのcompatibility shellを新設しない
- 検証:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `rg -n 'Holding|HeldBy|CollectSand|ZonePlace' docs --glob '*.md' --glob '!**/plans/**' --glob '!**/proposals/**'`
    （ゼロ件を要求せず、否定文・履歴説明を含め全hitをD01/D10の文脈で確認）
  - `rg -n 'NightTerror|Motivation|ApplyDeferred|TransportRequest|WorkType' docs --glob '*.md' --glob '!**/plans/**' --glob '!**/proposals/**'`
    （D02/D04/D09/D10の確認記録へ対応付ける）
  - `git diff --check`
- ロールバック境界:
  - D01～D12をdomain別docs変更単位にし、対応するcode milestoneと異なる契約へ戻さない。

## M9: 全体検証と計画archive

- 変更内容:
  - 各code milestoneでformat/package test/check/clippyを済ませた後、repo共通gateを実行する。
  - 数値不変条件は自動test、見た目と操作感は手動scenarioで確認する。
  - durable docsへ契約を移した後、本計画を
    `docs/plans/archive/implementation-spec-alignment-plan-2026-07-20.md`へ移し、索引を再生成する。
- 完了条件:
  - [x] M0～M8が完了している
  - [x] rust-analyzer診断にerrorがない
  - [x] workspaceのformat/check/clippy/test/docs/policy gateが通る
  - [x] Dream visual、Familiar設定、construction、door、Zone、UI actionの手動playtestで見た目と操作感に回帰がない
  - [x] 実装後最終ログと更新履歴が更新されている
  - [x] archive後の索引が最新で、archive pathが`git add -f`済みである
- 検証:
  - `python3 scripts/dev.py verify`
  - `git diff --check`
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 並行計画の差分へ古いbaselineを適用する | 正当な実装・docsを上書きする | M0を全milestoneのhard prerequisiteにし、重複diffと所有者を確定する |
| production driverを通さずstep単位でMessage発行する | catch-upでMessage列が増え、集約契約を破る | driver所有のframe accumulatorからloop後にSoulごと一件だけ発行する |
| RestArea退出後にsourceをQueryする | 最終drainのanchorを失う | producer時点でRestArea Entityとfallback originをsnapshotする |
| UI root/camera不在時に描画consumerが早期returnする | Dream massがsilentに失われる | 表示Query前のalways-on ingestionとdurable ledger、world reset testを置く |
| popupとUI particleのmassを加算する | 同じtransferを二重計上する | channelごとのledger debitを記録し、Poolとのoracleを明示する |
| ambient RestArea粒子までMessage駆動へ変える | visual密度契約が意図せず変わる | 獲得UIとambient world particleを別経路として明記・testする |
| release`0.0`を数値閾値として扱う | strict hysteresisを破る | `Option`等でrecruit無効を明示し、0.0 fixtureを置く |
| index利用とroot所有を同一視する | root shell規範違反を恒久化する | index挙動は維持し、実装をLeaf、登録をrootへ分離する |
| M3/M4を一括rollbackする | constructionとdoorの独立回帰を分離できない | 別milestone・別checkpointにする |
| focused filterが0 testで成功する | 回帰test未実行を見逃す | 必須test名を固定し、各packageの無filter`--lib` testも実行する |
| intent移行でgate/suppressionを失う | 背景操作、stale target、二重処理が起きる | producer gateとconsumer validationを分離してintegration testする |
| docs横断更新が意味的な漏れを起こす | 完了判定不能になる | D01～D12の監査表でcode owner・文書・状態を追跡する |

## 7. 検証計画

- 各code milestoneで必須:
  - `cargo fmt --all -- --check`
  - 変更crateの無filter`cargo test --locked -p <crate> --lib`
  - 必須テスト名のexact実行、またはmodule filter実行後に実行件数を確認
  - `cargo check --locked --workspace`
  - 変更crateの`cargo clippy --locked ... --all-targets -- -D warnings`
- docs milestoneで必須:
  - `python3 scripts/dev.py docs --write`
  - `python3 scripts/dev.py docs --check`
  - `git diff --check`
- 計画完了時:
  - `python3 scripts/dev.py verify`
  - `git diff --check`
- 手動確認シナリオ:
  1. Sleeping Soulの獲得popup/UI particleが最終drainでも視覚的に途切れず、数値massは自動test結果を正とする。
  2. RestAreaに1体・複数体を入れ、ambient粒子密度を維持しつつ、退出frameの獲得UIが不自然に消えない。
  3. Familiar設定`0.0`では新規recruitせず、`0.1`、`0.2`、`0.8`で加入直後のrelease loopがない。
  4. Floor/Wall constructionが必要tile完了時だけ遷移し、空site・別site・index不一致で誤遷移しない。
  5. unlocked doorが近接Soulで開閉し、Familiarだけでは開かず、locked doorは自動開閉しない。
  6. Zone配置を開始・drag・確定・Esc取消し、到達不能state削除後も表示と入力が一致する。
  7. Plant buildingのMove buttonを一度押し、BuildingMoveへ一度だけ遷移する。button上でもtargetを失わない。
  8. Architect categoryとdoor lockが一押下一副作用となる。
  9. Pause/modal/foreground UI上の操作が背後のMove/door/Architect actionを発火しない。
- パフォーマンス確認:
  - Dream transferは最大catch-upでもSoulごとにdriver一回一Message以下であることをcounter/testで確認する。
  - construction/doorはprofiling featureの候補数・tile数counterで全world scan非再導入を確認する。
  - frame-time比較が必要になった場合だけ、同一fixture/schemaで`python3 scripts/perf.py run`を使用する。

## 8. ロールバック方針

- M1～M8を別変更単位にし、Dream、Familiar、construction、door、AI ordering、Zone、UI、docs監査を
  独立して戻せるようにする。
- 各milestoneのdurable docsは対応するcodeと同じ変更単位に含める。M8で古い契約だけを戻さない。
- M1はMessage定義・登録・driver accumulator・ingestion ledger・resetを一単位として戻し、片側だけ残さない。
- M3とM4は別checkpointにし、未登録の全走査版を一時fallbackとして復活させない。
- M7はViewModel adapterと対象3 intent consumerを一単位として戻し、direct Queryと二重consumerを混在させない。
- shared worktreeでrollbackが必要な場合は、`git log --oneline -5`と対象`git diff HEAD -- <file>`を読み、
  並行作業由来でないことを確認する。`git checkout --`やdestructive resetを無断で使わない。
- commit権限がない作業ではmilestoneごとの対象diff一覧をcheckpointとして記録し、無関係差分を戻さない。
- 完了時は削除ではなくarchiveを選び、最終判断・検証ログを履歴として残す。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M0`～`M9`
- 未着手/進行中: `なし`
- 実装開始条件: `fdf045d5`をbaselineとするM0再照合は完了

### 次のAIが最初にやること

1. 実装、自動検証、手動playtestは完了済み。追加作業はない。

### ブロッカー/注意点

- actionable task dashboard関連のRust・README・durable docsは`fdf045d5`へ統合済みである。
  以後は同commit後のproduction経路を正とし、古い候補pathへ戻さない。
- M1のownerは未登録の単独systemではなく`slow_simulation_driver_system`である。
- RestArea sourceはVisualで`RestingIn`を再Queryせず、producer時点でsnapshotする。
- M2のrelease`0.0`はrecruit無効であり、数値`0.0`のrecruit閾値として扱わない。
- M3/M4はindex型自体を移さず、index利用system実装だけをLeafへ移す。
- M7では`ForegroundUiGate`をproducer、pointer suppressionとdomain再検証をconsumerの責務とする。
- Bevy 0.19のMessage/ordering APIは、実装時に現行コードまたは一次情報で再確認する。

### 参照必須ファイル

- `docs/invariants.md`
- `docs/dream.md`
- `docs/dream-visual.md`
- `docs/familiar_ai.md`
- `docs/ai-system-phases.md`
- `docs/architecture.md`
- `docs/cargo_workspace.md`
- `docs/crate-boundaries.md`
- `docs/plans/archive/actionable-task-dashboard-plan-2026-07-19.md`
- `crates/hw_soul_ai/src/soul_ai/update/slow_simulation.rs`
- `crates/hw_soul_ai/src/soul_ai/update/dream_update.rs`
- `crates/hw_soul_ai/src/soul_ai/update/rest_area_update.rs`
- `crates/hw_visual/src/dream/gain_visual.rs`
- `crates/hw_visual/src/dream/particle.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/recruitment.rs`
- `crates/hw_familiar_ai/src/familiar_ai/decide/state_decision/system.rs`
- `crates/hw_spatial/src/door_proximity.rs`
- `crates/bevy_app/src/interface/ui/interaction/intent_handler.rs`
- `crates/bevy_app/src/interface/ui/interaction/systems.rs`

### 計画作成・改訂時ベースライン

- `python3 scripts/dev.py docs --check`: `2026-07-20 / pass（8 current、47 archived、links pass）`
- `git diff --check`: `2026-07-20 / pass`
- 計画書のtrailing whitespace検査: `2026-07-20 / pass`
- `cargo check --workspace`: `2026-07-20 / 初版作成時pass。実装後は共通gateで再確認済み`
- 既知の並行状態:
  - actionable task dashboard実装commit: `fdf045d5`
  - shared worktreeの無関係なplan/proposal/archive差分は保存し、本計画の対象と混ぜて戻さない

### 実装後最終確認ログ

- 最終`python3 scripts/dev.py docs --check`: `pass（scoped staged snapshot: 7 current、49 archived、links/root index pass）`
- rust-analyzer workspace diagnostics: `0 errors / 0 warnings`
- 最終`cargo check --workspace --locked`: `pass`
- 最終`cargo clippy --workspace --all-targets --locked -- -D warnings`: `pass`
- 最終`cargo test --workspace --locked`: `pass`
- focused/package test: `M1～M7の必須回帰、Dream final/inactivity回帰、hw_core 13 tests、hw_familiar_ai 28 tests、hw_ui 44 testsがpass`
- 最終`python3 scripts/dev.py verify`: `pass（archive後、All quality gates passed）`
- 手動playtest: `2026-07-21 / pass（ユーザー確認。§7の9シナリオを6分類で受入済み）`
- 未解決エラー: `なし`

### Definition of Done

- [x] M0～M9が全て完了
- [x] DreamPool増分とtransfer/pending massの保存がproduction integration testで保証される
- [x] Familiarの`0.0`無効値と正の閾値のstrict hysteresisが境界testで保証される
- [x] construction/doorのindex利用実装がLeaf所有、root一意登録となる
- [x] AI orderingとZone stateがproduction testで固定される
- [x] UI hover boundaryと対象MenuActionの単一consumerが保証される
- [x] D01～D12が全て`Verified`
- [x] 影響ドキュメントが現行契約へ更新済み
- [x] `python3 scripts/dev.py verify`が成功
- [x] 計画がarchiveされ、docs索引が最新である

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-20` | `Codex` | 実装・仕様差異の再評価結果を基に初版作成 |
| `2026-07-20` | `Codex` | 自己レビューを反映。production driver、RestArea source/pending、0.0閾値、Leaf所有、並行作業M0、milestone分割、差異ID、検証・archive契約を修正 |
| `2026-07-20` | `Codex` | actionable task dashboard実装`fdf045d5`をbaselineとしてM0を開始 |
| `2026-07-20` | `Codex` | M0再監査を完了。Dream transfer ledger、Familiar hysteresis、AI ordering文書、Zone state除去の実装を開始 |
| `2026-07-20` | `Codex` | M1～M7の実装・回帰テストとM8のD01～D12仕様監査を完了。M9の全体gateとarchiveへ移行 |
| `2026-07-20` | `Codex` | M1/M2の必須テスト名とconsumer契約を再監査し、release/recruit APIの使い分けと全必須回帰を固定 |
| `2026-07-20` | `Codex` | rust-analyzer診断と`python3 scripts/dev.py verify`がpass。手動playtest未実施の制約を記録し、archive後の最終gateへ移行 |
| `2026-07-20` | `Codex` | 計画をarchiveして索引を9 current / 49 archivedへ更新。archive後の`python3 scripts/dev.py verify`もpassし、M0～M9を完了 |
| `2026-07-21` | `Codex` | ユーザーによる手動playtestでDream visual、Familiar設定、construction、door、Zone、UI actionの全確認項目がpass |
| `2026-07-21` | `Codex` | commit前レビューでDream ledgerのslow-step間誤flushを検出。producer確定の`is_final`と0.5秒無通信判定へ修正し、回帰テスト・現行仕様を同期 |
