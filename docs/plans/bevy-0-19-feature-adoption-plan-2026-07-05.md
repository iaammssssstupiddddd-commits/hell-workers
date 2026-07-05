# Bevy 0.19 新機能活用計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `bevy-0-19-feature-adoption-plan-2026-07-05` |
| ステータス | `Draft` |
| 作成日 | `2026-07-05` |
| 最終更新日 | `2026-07-05` |
| 作成者 | Claude (調査ベース) |
| 関連提案 | N/A |
| 関連Issue/PR | 前提: `docs/plans/bevy-0-19-migration-plan-2026-07-05.md`（移行完了済み） |

## 1. 目的

- 解決したい課題: Bevy 0.19 移行は完了したが、0.19 新機能の活用は Out of Scope として見送られたまま。0.18 時代のパターン（自前タイマー tick システム、自前スクロール実装など）が残っている。
- 到達したい状態: 効果が明確な箇所のみ 0.19 ネイティブなパターンに置き換わり、boilerplate と自前実装の保守コストが減っている。
- 成功指標: `cargo check` / `cargo clippy --workspace`（警告 0）成功、置き換え箇所の挙動回帰なし、M3 は計測で改善を確認（改善なしなら差し戻し）。

## 2. スコープ

### 対象（In Scope）

- **M1**: fire-and-forget なワンショット Timer の Delayed Commands（`commands.delayed().secs(..)`）置き換え
- **M2**: hw_ui 自前スクロール（`UiScrollArea`）の `bevy_ui_widgets` 正式ウィジェット置き換え + EditableText 使用方針のドキュメント化
- **M3**: `contiguous_iter` による dream パーティクル更新の SIMD 最適化（計測ベース）
- **M4**: ICU4X 日本語セグメンテーション問題の目視 QA（移行計画からの持ち越し）とドキュメント反映

### 非対象（Out of Scope）

- **BSN（`bsn!` マクロ）による hw_ui 書き換え**: `fn spawn_*` 36 個 + 階層スポーン 12 ファイルと範囲が広大。「新規 UI から試験導入 → 良ければ別計画で既存移行」とし、本計画では扱わない
- **Resources-as-Components の hook/observer 活用**: `is_changed()` 49 箇所は run condition として今もイディオマティック。即時反応が必要な具体的課題が出た時に個別対応
- **Observer run conditions（`ObserverWithCondition`）**: TaskMode 等でガードしている observer は 2 箇所程度で効果薄。見つけたら都度対応で十分
- **Diagnostics Overlay / Text Gizmos**: dev_panel はゲーム固有情報が主で置き換え対象が薄い
- **セーブ基盤（Handle Serialization / DynamicWorld / Asset Saving / SettingsPlugin）**: セーブ機能自体が未着手（workspace に serde 使用 0 件）。セーブ機能の計画時にこれらを前提とすること（自前設計しない）
- **FontSize レスポンシブ単位（Vh/Rem）**: UI スケーリング対応をやる時の選択肢。単体では動機なし

## 3. 現状とギャップ

- 現状: Bevy 0.19 でコンパイル・動作するが、コードパターンは 0.18 時代のまま（Timer コンポーネント + tick システム、自前 `UiScrollArea`、スカラーループのパーティクル更新）
- 問題: ワンショット遅延のたびにコンポーネント + tick システムの boilerplate が必要。自前ウィジェットは公式提供と重複した保守コスト。dream パーティクルは既知の性能課題（`dream-bubble-perf-2026-04-09.md`）
- 本計画で埋めるギャップ: 効果とリスクのバランスが良い 3 領域に絞って 0.19 ネイティブ化する

## 4. 実装方針（高レベル）

- 方針: マイルストーンごとに独立して実装・検証・コミット可能にする。M3 は計測が改善を示さなければ差し戻す
- 設計上の前提: 各 API は docs.rs の 0.19.0 で確認済み（§4.1）。推測で書かない
- Bevy 0.19 API での注意点: **`DelayedCommands` はキャンセルハンドルを返さない fire-and-forget 型**。キャンセル・リセット・一時停止が必要なタイマーは置き換え対象外（§M1 適格判定基準）

### 4.1 検証済み 0.19 API（docs.rs で確認、2026-07-05）

| API | 場所 | 要点 |
| --- | --- | --- |
| `DelayedCommandsExt::delayed()` | `bevy_time::delayed_commands` | `Commands` の拡張トレイト。`commands.delayed().secs(f32)` / `.duration(Duration)` が遅延適用される `Commands` を返す。Drop 時に `DelayedCommandQueue` エンティティとして spawn され `check_delayed_command_queues` システムが発火。**キャンセル手段なし** |
| `ContiguousQueryData` / `QueryNotDenseError` | `bevy_ecs::query` | `contiguous_iter` は dense（Table ストレージ）なクエリのみ。非 dense だと `QueryNotDenseError` |
| `ScrollAreaPlugin` / `ScrollbarPlugin` / `ScrollbarTemplate` | `bevy_ui_widgets::{scrollarea,scrollbar}` | スクロール領域とスクロールバーの公式提供 |
| `EditableText` / `EditableTextInputPlugin` / `SelectAllOnFocus` | `bevy_ui_widgets::text_input` | 公式テキスト入力。IME 対応（`ImeSystems`）あり |
| `ObserverWithCondition` | `bevy_ecs::observer::condition` | observer への run condition（本計画では対象外、参考） |

## 5. マイルストーン

## M1: fire-and-forget タイマーの Delayed Commands 置き換え

- 変更内容: `TimerMode::Once` のうち適格なものを `commands.delayed().secs(..)` に置き換え、対応する tick システム・コンポーネントを削除する

### 適格判定基準（4 つすべて満たすこと）

1. `TimerMode::Once` である
2. 発火まで tick が**無条件**（状態によって tick を止めない）
3. 途中キャンセル・リセットの経路が**存在しない**
4. 発火時に必要なデータが、コマンド発行時に確定しているか、カスタムコマンド内で World から取得できる

### 調査済みの分類

| 箇所 | 判定 | 理由 |
| --- | --- | --- |
| `ReactionDelay`（`hw_visual/src/speech/observers.rs:202,375` 挿入 / `:268` tick / `components.rs:86` 定義） | ✅ **確定候補** | 挿入 → 0.3 秒後に発火 → remove のみ。キャンセル経路なし。発火時に `GlobalTransform` を読むため、closure コマンド内で World から取得する形にする |
| `ItemDespawnTimer`（`hw_logistics/src/item_lifetime.rs`） | ❌ 対象外 | 予約中（`ReservedForTask`/`LoadedIn` 等）は tick を止める条件付きタイマー（基準 2 違反） |
| `DoorCloseTimer`（`hw_jobs/src/model.rs:322` / `hw_world/src/door_systems.rs`） | ❌ 対象外 | Soul 近接で remove されるキャンセル型（基準 3 違反） |
| tooltip `delay_timer`（`hw_ui/src/interaction/tooltip/system.rs:180`） | ❌ 対象外 | マウス移動でリセット（基準 3 違反） |
| drag `hold_timer`（`hw_ui/src/list/drag_state.rs`） | ❌ 対象外 | リリースでキャンセル（基準 3 違反） |
| 会話ターン（`hw_visual/src/speech/conversation/phase_handlers.rs`） | 🔍 要個別調査 | 状態機械のターン制御。会話中断経路の有無を確認してから判定 |
| `hw_soul_ai`（refine.rs / sand_collect.rs / drifting.rs / escaping.rs / unloading.rs） | 🔍 要個別調査 | タスク中断（`unassign_task`）で消える可能性が高く、多くは対象外の見込み |
| `hw_core`（population.rs / gathering.rs）、`hw_world`（room_detection/ecs.rs）、`resource_sync.rs`、`blueprint_auto_gather.rs` | 🔍 要個別調査 | 未精査 |

- 変更ファイル:
  - `crates/hw_visual/src/speech/observers.rs`（ReactionDelay 挿入 2 箇所 + tick システム削除）
  - `crates/hw_visual/src/speech/components.rs`（ReactionDelay 定義削除）
  - `crates/hw_visual/src/speech/mod.rs`（システム登録解除）
  - 要個別調査分は判定後に追記
- 完了条件:
  - [ ] 🔍 の全箇所を適格判定基準で分類し、本表を更新
  - [ ] 適格箇所の置き換えと tick システム・コンポーネントの削除（No Dead Code ルール）
  - [ ] 恐怖/疲労リアクション（😨/😓 バブル）が従来どおり約 0.3 秒遅延で Soul の現在位置に表示される
- 検証:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` + `cargo clippy --workspace`（警告 0）
  - 起動して Soul の脅威/疲労リアクションを目視確認

## M2: ui_widgets 正式ウィジェットへの置き換え

- 変更内容:
  1. 自前 `UiScrollArea`（`hw_ui/src/components.rs:257`）とそのスクロール処理を `bevy_ui_widgets::scrollarea` / `scrollbar` に置き換え。現状スクロールバーの視覚表示がない場合は `ScrollbarTemplate` で追加（UX 改善）
  2. `IgnoreScroll` 等の付随コンポーネントの要否を再評価（公式側で代替できるなら削除）
  3. **EditableText 使用方針のドキュメント化**: Soul 名リネーム・検索 UI 等のテキスト入力を将来実装する際は `bevy_ui_widgets::text_input::EditableText` を使い自前実装しない旨を `crates/hw_ui/CLAUDE.md`（`_rules.md`）に追記
- 変更ファイル:
  - `crates/hw_ui/src/components.rs`（UiScrollArea 削除）
  - `crates/hw_ui/src/setup/entity_list.rs`
  - `crates/bevy_app/src/interface/ui/list/interaction/navigation.rs`
  - `crates/hw_ui/src/plugins/`（プラグイン登録。`UiWidgetsPlugins` は 0.19 で DefaultPlugins に統合済みのため二重登録に注意 — 移行計画 M3 で PopoverPlugin 二重登録を検出した前例あり）
  - `crates/hw_ui/CLAUDE.md` / `crates/hw_ui/_rules.md`（EditableText 方針）
- 完了条件:
  - [ ] エンティティリストのホイールスクロールが従来どおり動作（`Scroll: Mouse Wheel` ヒント表示含め挙動確認）
  - [ ] `UiScrollArea` と関連の自前スクロールシステムが削除されている
  - [ ] EditableText 方針が hw_ui のルールファイルに記載されている
- 検証:
  - `cargo check` + `cargo clippy --workspace`（警告 0）
  - 起動してエンティティリストのスクロール・ドラッグ操作を目視確認

## M3: contiguous_iter によるパーティクル更新最適化（計測ベース）

- 変更内容: `ui_particle_update_system`（`hw_visual/src/dream/ui_particle/update.rs:31`）の `q_particles.iter_mut()` ループを `contiguous_iter` ベースに書き換え。AVX2 環境で SIMD 化される（0.19 リリースノートで約 3 倍の実測報告）
- 前提調査（実装前に必ず実施）:
  1. **計測**: 現状のパーティクル大量発生時のシステム実行時間を計測（`dream-bubble-perf-2026-04-09.md` の再現手順を参照）。そもそもボトルネックでなければ本 M はスキップ
  2. **dense 要件確認**: 対象クエリ（`Entity`, `&mut particle`, `&mut Node`, material handle, `&mut Transform` 混在）が `ContiguousQueryData` を満たすか確認。`Option<&T>` や sparse ストレージ混在で `QueryNotDenseError` になる場合、クエリ分割か対象コンポーネント限定（数値演算部分のみ）を検討
- 変更ファイル:
  - `crates/hw_visual/src/dream/ui_particle/update.rs`
  - `crates/hw_visual/src/dream/ui_particle/update/update_standard.rs`
- 完了条件:
  - [ ] 前後計測で改善を確認（改善なし・微小なら差し戻して本 M を「見送り」と記録）
  - [ ] パーティクルの見た目（軌道・マージ・吸収演出）に回帰なし
- 検証:
  - `cargo check` + `cargo clippy --workspace`（警告 0）
  - dream バブル大量発生シナリオでの FPS / システム実行時間の前後比較

## M4: ICU4X 日本語折返し目視 QA とドキュメント反映

- 変更内容（移行計画 M3「既知の制約」の持ち越し）:
  1. bevy_text / parley の 0.19.x パッチで `ICU4X data error: No segmentation model for language: ja` が解消されていないか確認（`cargo update -p bevy` 相当の patch 更新確認）
  2. tooltip / dialog / task_list 等、長い和文を表示する画面で折返しの目視 QA（**ユーザーの画面確認が必要**）
  3. 実害があれば `TextLayout::linebreak` の設定で緩和できないか調査
  4. 本計画完了時に成果を `docs/` の恒久ドキュメントへ反映（`hell-workers-update-docs` の対象判定に従う）
- 変更ファイル:
  - （QA 結果次第）`crates/hw_ui/**` のテキストレイアウト設定
  - `docs/architecture.md` 等（採用したパターンの記録）
- 完了条件:
  - [ ] 日本語折返しの実害有無が判定され、記録されている
  - [ ] 採用した 0.19 パターン（Delayed Commands の適格判定基準など）が恒久ドキュメントに反映されている
- 検証: 目視 QA + `cargo check`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Delayed Commands 化した箇所に後からキャンセル要件が発生 | 再度 Timer 方式へ戻す手戻り | 適格判定基準 3（キャンセル経路なし）を厳格適用。疑わしきは対象外 |
| `DelayedCommandQueue` がエンティティとして spawn される | `Query<Entity>` 系の広いクエリに混入 | 移行計画で Resources-as-Components 対応時に顕在化しなかった実績あり。broad query の panic が出たらフィルタ追加 |
| ScrollArea 置き換えで操作感が変わる（速度 28.0 等の自前チューニング） | UX 劣化 | 公式ウィジェットのパラメータで同等感を再現。無理なら M2 のスクロール置換のみ見送り（EditableText 方針記載は独立して実施） |
| `contiguous_iter` の dense 要件を満たせない | M3 が空振り | 前提調査を実装前に必須化。クエリ分割で部分適用も検討 |
| ui_widgets プラグイン二重登録 | 起動時 panic | 移行計画 M3 の PopoverPlugin 前例を参照。`DefaultPlugins` に統合済みのものを個別 add しない |

## 7. 検証計画

- 必須:
  - `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check`
  - `cargo clippy --workspace`（警告 0）
- 手動確認シナリオ: 起動 → Soul 脅威/疲労リアクション表示（M1）→ エンティティリストスクロール（M2）→ dream バブル大量発生（M3）→ 長い和文 UI の折返し（M4）
- パフォーマンス確認（M3）: パーティクル更新システムの実行時間とフレームレートを置き換え前後で比較

## 8. ロールバック方針

- どの単位で戻せるか: マイルストーン単位（各 M を独立コミットにする）
- 戻す時の手順: 該当コミットの revert。M3 は計測不合格時に計画内で差し戻す前提

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（計画作成のみ、実装未着手）
- 完了済みマイルストーン: なし
- 未着手/進行中: M1〜M4 すべて未着手。着手順は M1 → M2 → M3 → M4 を推奨（M4 のパッチ確認だけは随時可）

### 次のAIが最初にやること

1. §4.1 の検証済み API 表を信頼してよい（docs.rs の bevy_time / bevy_ecs / bevy_ui_widgets 0.19.0 で確認済み）。再調査不要
2. M1 の「🔍 要個別調査」箇所を適格判定基準（§M1）で分類し、表を更新してから実装に入る
3. 確定候補 `ReactionDelay` から着手（最小・独立・検証容易）

### ブロッカー/注意点

- **`DelayedCommands` にキャンセル API はない**。キャンセルが要るタイマーを置き換えないこと（M1 の ❌ 行は調査済みの結論なので蒸し返さない）
- `ReactionDelay` の発火処理は Soul の現在位置（`GlobalTransform`）を発火時点で読む。遅延コマンドは closure コマンド（World アクセス可）で書くこと。対象 Soul が発火前に despawn される可能性に注意（`try_` 系 / `get_entity` で防御）
- M4 の目視 QA はユーザーの画面確認が必要（サンドボックスからスクリーンショット不可の前例あり）
- CLAUDE.md「No Dead Code」ルール: 置き換えたら旧 Timer コンポーネント・tick システムを必ず削除

### 参照必須ファイル

- `docs/plans/bevy-0-19-migration-plan-2026-07-05.md`（移行の実施ログ。特に M3 の PopoverPlugin 二重登録と ICU4X 既知の制約）
- `docs/plans/dream-bubble-perf-2026-04-09.md`（M3 の計測手順）
- `crates/hw_visual/src/speech/observers.rs`（M1 確定候補）
- `crates/hw_ui/src/setup/entity_list.rs`（M2 対象）

### 最終確認ログ

- 最終 `cargo check`: 未実施（実装未着手）
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M4 完了（M3 は「計測不合格で見送り」も完了と見なす）
- [ ] `cargo check` / `cargo clippy --workspace`（警告 0）成功
- [ ] 採用パターンが恒久ドキュメントに反映され、本計画をアーカイブ可能

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-05` | Claude | 初版作成（0.19 新機能の活用候補調査 + docs.rs での API 検証に基づく） |
