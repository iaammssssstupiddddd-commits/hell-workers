# AI デバッグラベル + 計測補助 — gizmo テキスト導入計画（dev_tools 再検証済み）

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `dev-tools-debug-overlay-plan-2026-07-05` |
| ステータス | `Draft` |
| 作成日 | `2026-07-05` |
| 最終更新日 | `2026-07-05` |
| 作成者 | Claude (調査ベース) |
| 関連提案 | N/A |
| 関連Issue/PR | 関連: `performance-cpu-2026-04-16.md`（M3 のフレームタイムグラフ） |

## 0. 再検証の結論（2026-07-05）: 自前実装は置き換えない

初版は `FpsOverlayPlugin` / `DiagnosticsOverlayPlugin` の導入を M1 に置いていたが、既存実装との比較再検証により**置き換えメリットなし**と判断し、スコープから外した。根拠:

1. **削減対象がほぼない**: 自前 FPS カウンタ（`hw_ui/src/interaction/status_display/runtime.rs:7-33`）は Local 変数 + 1 秒平均の約 30 行で完結し、要件を満たしている。Bevy diagnostics 基盤（`FrameTimeDiagnosticsPlugin` / `DiagnosticsStore`）への依存もゼロで、置き換えても削れるコードが 30 行しかない
2. **配置の退行**: `FpsOverlayPlugin` のオーバーレイは registry ソース（`fps_overlay.rs:176-211`）で確認した通り `PositionType::Absolute`（オフセット設定なし = 左上固定）+ `GlobalZIndex(FPS_OVERLAY_ZINDEX)` で最前面描画され、**位置を設定する手段が `FpsOverlayConfig` にない**。DevPanel は左上にあり、過去に「独立 FPS widget が DevPanel と重なって不可視になったため DevPanel に統合した」経緯（`docs/debug-features.md` FPS インジケーター節）が明記されている。導入はこの修正の逆行になる
3. **DevPanel の大半は代替不能**: dev_panel.rs（689 行）の主内容は RTT 品質切替・3D 固定費トグル・LOD インジケーター・Instant Build 等のゲーム固有機能で、`bevy_dev_tools` に対応物がない。置き換えても DevPanel は消えない
4. **`DiagnosticsOverlayPlugin` も同様**: エンティティ数等を出すには別途 diagnostics プラグイン登録が必要で、既存 DevPanel に自前行を 1 行足す方が統合的

残る正味の価値は以下の 2 つで、いずれも「置き換え」ではなく**新規能力**:

- **gizmo テキストによるワールド内 AI デバッグラベル**（本計画の主軸。`bevy_dev_tools` 不要 — `bevy_gizmos` は既存 feature で有効）
- **フレームタイムグラフ**（`FrameTimeGraphConfig`、シェーダー描画によるスパイク可視化）— 自前実装は高コストで、1 秒平均 FPS では見えないヒッチを可視化できる唯一の項目。ただし常設せず、performance-cpu 計画の計測時に導入判断する（M3・オプション）

## 1. 目的

- 解決したい課題: Soul / Familiar の AI 状態（AssignedTask・フェーズ・Squad 状態）をワールド内で直接確認できず、デバッグがログ頼み。フレームスパイクの可視化手段がない
- 到達したい状態: DevPanel からトグルできるワールド内 AI ラベルがあり、必要時にフレームタイムグラフで計測できる
- 成功指標: `cargo check` / `cargo clippy --workspace`（警告 0）成功。既存 DevPanel / FPS 表示は無変更

## 2. スコープ

### 対象（In Scope）

- **M1**: gizmo テキスト（`Gizmos::text_2d` 系）による Soul / Familiar AI 状態のワールド内ラベル
- **M2**: DevPanel へのトグル行追加（**新規 UI 部分は BSN + `Checkbox`**）
- **M3**（オプション・計測時のみ）: `bevy_dev_tools` feature + `FpsOverlayPlugin` によるフレームタイムグラフの一時導入

### 非対象（Out of Scope）

- **自前 FPS カウンタ / DevPanel の置き換え**（§0 の再検証で却下。蒸し返さない）
- `DiagnosticsOverlayPlugin` の導入（DevPanel への行追加で代替可能）
- `bevy_feathers` / `easy_screenshot` / `screenrecording`（動機なし。必要になったら都度）

## 3. 現状とギャップ

- 現状: FPS は自前カウンタで DevPanel に統合済み。AI 状態の可視化はゼロ。gizmos は `2d` feature（`bevy_gizmos_render` 内包）で有効
- 問題: soul_ai / familiar_ai のデバッグで「今この Soul が何をしているか」を見るのにログ突き合わせが必要
- 本計画で埋めるギャップ: 0.19 の gizmo テキスト（ストロークフォント内蔵・フォントアセット不要）で AI ラベルを最小工数で実現する

## 4. 実装方針（高レベル）

- 方針: ラベル描画は `#[cfg]` ゲートせず通常ビルドに含め、`DebugVisible` 系 Resource + run condition でトグルする（既存 debug-features のパターン踏襲。gizmo は immediate mode なのでオフ時のコストは run condition で止めればゼロ）
- 設計上の前提: ラベル文字列は英数字のみ（enum バリアント名）。ストロークフォントのグリフ対応が ASCII 想定のため
- **BSN 前提**: M2 の新規トグル行 UI は `bsn!` + `commands.spawn_scene` で構築（BSN 制約は `save-load-world-serialization-plan-2026-07-05.md` §4.1 参照）

### 4.1 検証済み 0.19 API（registry ソースで確認、2026-07-05）

| API | 場所 | 要点 |
| --- | --- | --- |
| `Gizmos::text` / `text_sections` / `text_2d` / `text_sections_2d` | `bevy_gizmos/src/stroke_text.rs:197,239,281,323` | ストロークフォント内蔵（`simplex_stroke_font.rs`）。フォントアセット不要。追加 feature 不要 |
| `Checkbox` / `ValueChange<bool>` | `bevy_ui_widgets/src/checkbox.rs:38`, `lib.rs:90` | M2 のトグル行。`UiWidgetsPlugins` は DefaultPlugins 登録済み |
| `FpsOverlayPlugin` / `FpsOverlayConfig` / `FrameTimeGraphConfig` | `bevy_dev_tools/src/fps_overlay.rs:54,110,140` | M3 専用。`enabled` と `frame_time_graph_config.enabled` は独立トグル可。**表示位置は設定不可（左上固定・最前面）** — DevPanel と重なるため常設しない |
| `bevy_dev_tools` feature | `bevy/Cargo.toml:2667` | M3 実施時のみ追加 |

## 5. マイルストーン

## M1: Soul / Familiar AI 状態のワールド内ラベル

- 変更内容:
  1. `crates/bevy_app/src/systems/debug/ai_labels.rs`（新規）: `gizmos.text_2d`（3D 位置基準なら `text`）で Soul 上に `AssignedTask` 種別・AI フェーズを、Familiar 上に Squad 状態・TaskMode を描画
  2. 表示モード: OFF / 選択中のみ / 全表示 の 3 段トグル（全表示は画面内カリング必須 — 既存の spatial grid か camera viewport 判定を流用）
  3. トグル Resource（`DebugAiLabels` 等）を定義し、run condition でシステムごと停止
  4. 実装前にストロークフォントの対応グリフを `simplex_stroke_font.rs` で確認し、表示文字列を対応範囲に限定
- 変更ファイル:
  - `crates/bevy_app/src/systems/debug/ai_labels.rs`（新規）
  - `crates/bevy_app/src/plugins/interface_debug.rs`（登録）
  - `docs/debug-features.md`（機能の記録）
- 完了条件:
  - [ ] Soul 頭上にタスク種別が表示され、タスク遷移に追従する（ユーザー目視 QA）
  - [ ] OFF 時はシステムが走らない（run condition）
- 検証: `cargo check` + 起動して Soul のタスク遷移を目視

## M2: DevPanel トグル行（BSN + Checkbox）

- 変更内容:
  1. DevPanel に「AI Labels: OFF/Sel/All」トグル行を追加
  2. **新規行の UI は `bsn!` で構築**し、`Checkbox` + `ValueChange<bool>` observer（3 段トグルなら既存ボタンパターンでも可 — 実装時に UI として自然な方を選び、BSN は維持）
  3. 本プロジェクト初の Checkbox / DevPanel 内 BSN 採用として知見を `hw_ui/_rules.md` に記録
- 変更ファイル:
  - `crates/bevy_app/src/interface/ui/dev_panel.rs`
  - `crates/hw_ui/_rules.md`
- 完了条件:
  - [ ] DevPanel から M1 の表示モードを切り替えられる
- 検証: `cargo check` + `cargo clippy --workspace`（警告 0）+ 起動確認

## M3（オプション）: フレームタイムグラフの一時導入

- 実施条件: `performance-cpu-2026-04-16.md` の計測でフレームスパイクの可視化が必要になった時のみ。常設しない
- 変更内容:
  1. `Cargo.toml` に `bevy_dev_tools` feature を追加（計測ブランチ/一時コミットで可）
  2. `FpsOverlayPlugin` を追加し、`FpsOverlayConfig { enabled: false（テキスト側は必要に応じ）, frame_time_graph_config.enabled: true, .. }` でグラフ主体に使う。左上の DevPanel と重なるため、計測中のみ DevPanel を最小化するか重なりを許容する（一時利用なので作り込まない）
  3. 計測完了後に revert するか、常設したくなった場合は本計画を更新して判断を記録する
- 完了条件:
  - [ ] （実施した場合）スパイク箇所の特定に使えたか、結果を performance-cpu 計画に記録
- 検証: `cargo check`（feature 有/無）

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| ストロークフォントのグリフが想定より狭い（記号等が出ない） | ラベル表記の制約 | M1 冒頭でグリフ表を確認し、表示文字列設計をそれに合わせる |
| 全 Soul ラベル描画が重い | dev 時の FPS 低下 | 画面内カリング + 「選択中のみ」をデフォルトモードに |
| gizmo テキストが RtT / 2D 合成レイヤーと描画順で干渉 | ラベルが埋もれる | gizmo は最前面描画が基本だが、本プロジェクトは RtT 合成があるため M1 で最初に 1 体表示して確認 |
| M3 のオーバーレイが DevPanel と重なる | 一時的な視認性低下 | 計測中のみの割り切り（§0 の理由で常設はしない） |

## 7. 検証計画

- 必須: `CARGO_HOME=/home/satotakumi/.cargo CARGO_TARGET_DIR=target cargo check` + `cargo clippy --workspace`（警告 0）
- 手動確認シナリオ: 起動 → DevPanel でラベル ON → Soul 選択でラベル表示 → タスク遷移で文字列が変わる → OFF で消える
- ラベルの見た目・重なりはユーザー目視 QA が必要

## 8. ロールバック方針

- どの単位で戻せるか: マイルストーン単位。M3 は最初から一時導入前提
- 戻す時の手順: 該当コミット revert

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（計画作成 + 再検証のみ）
- 未着手/進行中: M1〜M2 未着手。M3 は performance-cpu 計画の必要時まで保留

### 次のAIが最初にやること

1. §0 の再検証結論を前提にする。**FpsOverlayPlugin / DiagnosticsOverlayPlugin での置き換えを再提案しない**（位置設定不可・DevPanel 重なりの経緯まで確認済み）
2. `simplex_stroke_font.rs` のグリフ対応を確認してから M1 の表示文字列を設計
3. `Gizmos::text_2d` のシグネチャ（座標系・アンカー・スケール指定）を `stroke_text.rs:197-323` で精読

### ブロッカー/注意点

- gizmo は immediate mode: 毎フレーム描画呼び出しが必要。run condition で止めればオフ時コストゼロ
- RtT 合成パイプラインとの描画順は実機確認が必要（リスク表参照）
- CLAUDE.md「No Dead Code」: M3 を実施しない限り `bevy_dev_tools` feature を追加しない

### 参照必須ファイル

- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_gizmos-0.19.0/src/stroke_text.rs`（一次情報）
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_gizmos-0.19.0/src/simplex_stroke_font.rs`（グリフ）
- `crates/bevy_app/src/interface/ui/dev_panel.rs` / `crates/bevy_app/src/plugins/interface_debug.rs`
- `docs/debug-features.md`

### 最終確認ログ

- 最終 `cargo check`: 未実施（実装未着手）
- 未解決エラー: なし

### Definition of Done

- [ ] M1〜M2 完了（M3 は「未実施」のままでも完了と見なす）
- [ ] `cargo check` / `cargo clippy --workspace`（警告 0）成功
- [ ] `docs/debug-features.md` にラベル機能とトグルが記載され、本計画をアーカイブ可能

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-05` | Claude | 初版作成（bevy_dev_tools / stroke_text gizmos の registry ソース検証に基づく） |
| `2026-07-05` | Claude | 再検証により FpsOverlay/DiagnosticsOverlay 置き換えを却下（§0）。gizmo AI ラベル主軸に再構成、フレームタイムグラフはオプション M3 に降格 |
