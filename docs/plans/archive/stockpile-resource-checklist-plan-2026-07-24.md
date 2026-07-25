# Stockpile受入資材チェックリスト実装計画

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `stockpile-resource-checklist-plan-2026-07-24` |
| ステータス | `Archived` |
| 作成日 | `2026-07-24` |
| 最終更新日 | `2026-07-25` |
| 作成者 | `Codex` |
| 関連提案 | `docs/proposals/gameplay-management-improvements-proposal-2026-07-17.md` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: Stockpileの受入資材を単一ボタンで順送りするため、目的の設定へ辿り着きにくく、複数資材を許可できない。
- 到達したい状態: RimWorld型の一覧で資材ごとの許可を直接ON/OFFし、全選択・全解除と複数選択を単一セル・矩形編集・save/loadで一貫して扱える。
- 成功指標:
  - 全資材が常時見えるチェックリストとして表示され、各行を1回押すだけで個別に切り替えられる。
  - 複数許可集合を物流の全経路が同じ `accepts` 契約で評価する。
  - B1実装済みsaveの `Any` / `Only(ResourceType)` を引き続きロードできる。

## 2. スコープ

### 対象（In Scope）

- `StockpileAcceptance` の複数資材集合と正規化API。
- Info Panelの全選択、全解除、資材別チェックリスト。
- 単一セル変更、矩形への方針コピー、Draining、committed搬送の既存契約維持。
- Reflect save schema、round-trip、旧 `Any` / `Only` 互換。
- 物流、Info Panel、save/load、受入計画の文書同期。

### 非対象（Out of Scope）

- 資材カテゴリの階層ツリー、検索、折り畳み、プリセット名。
- `ResourceType` 自体の追加・削除。
- B1-R05のworld replacement後の旧情報パネル残留修正。
- B2 Familiar方針、B3 Soul Energy制御。

## 3. 現状とギャップ

- 現状: durable `StockpileAcceptance` は `Any / Only(ResourceType)` の2形態で、Info Panelは全資材を順送りする単一ボタンを持つ。
- 問題: 9資材では操作回数が多く、現在値以外の候補が見えず、複数資材の許可を表現できない。
- 本計画で埋めるギャップ:
  - 旧variantを残したまま複数選択variantを追加し、意味的に同じ集合を正規化する。
  - 9資材を2列の一覧として表示し、ASCIIの `[x] / [ ]` と直接操作で状態を明示する。
  - 一覧が増えても既存のpriority、target、export、範囲編集を同じInfo Panel内で操作可能にする。

## 4. 実装方針（高レベル）

- 方針:
  - `StockpileResourceSet` を固定bit集合として `hw_logistics` に置き、資材列挙と集合演算の唯一の正本にする。
  - `StockpileAcceptance::Any / Only` は旧save互換のため維持し、`Selected(set)` を追加する。
  - 全許可は `Any`、単一許可は `Only`、0件または2〜8件は `Selected` へ正規化する。
  - UIは値を直接書かず、既存 `StockpilePolicyPatch` とtyped intent/outcome経路を再利用する。
- 設計上の前提:
  - 1セルは同時に1種類だけを格納する既存物理契約を維持する。複数許可は「空になった後に受け入れられる候補集合」であり、混載を許可しない。
  - committed搬送は方針変更後もgrandfatherし、許可解除済みの既存在庫はDrainingへ遷移する。
  - 新variantを含むsaveを旧実行ファイルが読めるforward compatibilityは保証しない。
- Bevy 0.19 APIでの注意点:
  - 静的Info Panel treeを起動時に生成し、`MenuButton`のactionとTextだけをlive ViewModelから更新する。
  - 2列一覧には既存Bevy 0.19の `FlexWrap::Wrap` を使い、動的spawn/despawnを毎更新で行わない。
  - 固定pxへ倍率を掛ける `UiScale` を考慮し、panel高はviewportの58%を上限として縦scrollを許可する。

## 5. マイルストーン

## M1: 複数資材集合と保存互換

- 変更内容: 集合型、正規化、toggle、Reflect登録、save round-tripを追加する。
- 変更ファイル:
  - `crates/hw_logistics/src/zone.rs`
  - `crates/hw_logistics/src/lib.rs`
  - `crates/bevy_app/src/systems/save/schema.rs`
  - `crates/bevy_app/src/systems/save/schema/tests.rs`
- 完了条件:
  - [x] 0件、単一、複数、全件の集合を表現できる。
  - [x] 旧 `Any / Only` の意味とRON表現を維持する。
  - [x] 複数選択がDynamicWorldをround-tripする。
- 検証:
  - `cargo test -p hw_logistics --lib`
  - `cargo test -p bevy_app@0.1.0 --lib stockpile_policy_round_trip`

## M2: RimWorld型チェックリストUI

- 変更内容: cycleボタンを全選択・全解除・資材別toggleへ置換し、選択数と各行の状態をlive更新する。
- 変更ファイル:
  - `crates/hw_ui/src/components.rs`
  - `crates/hw_ui/src/panels/info_panel/layout.rs`
  - `crates/hw_ui/src/panels/info_panel/model.rs`
  - `crates/hw_ui/src/panels/info_panel/update.rs`
- 完了条件:
  - [x] 全資材が一覧で見える。
  - [x] 各資材、全選択、全解除が既存typed intentを発行する。
  - [x] 矩形編集が集合全体をコピーする。
  - [x] cycle関数と受入資材の `(cycle)` 表示が残らない。
  - [x] `1280x720 / UiScale 1.25` の高さ予算内でpanelを制限し、下段へscrollできる。
- 検証:
  - `cargo test -p hw_ui --lib`
  - `cargo test -p bevy_app@0.1.0 --lib`

## M3: 横断回帰と文書同期

- 変更内容: 複数許可の物流・Draining・save回帰を追加し、恒久仕様と実機受入項目を更新する。
- 変更ファイル:
  - `crates/hw_logistics/src/stockpile_policy.rs`
  - `docs/logistics.md`
  - `docs/info_panel_ui.md`
  - `docs/save_load.md`
  - `docs/invariants.md`
  - `docs/plans/stockpile-policy-manual-acceptance-plan-2026-07-23.md`
- 完了条件:
  - [x] 許可集合内の資材だけが新規搬入できる。
  - [x] 許可解除した既存在庫がDrainingになり、集合内ならDrainingにならない。
  - [x] 全解除した空セルは `Disabled` と表示し、全資材を拒否する。
  - [x] 実機再確認項目がチェックリスト操作と複数選択round-tripを含む。
  - [x] 全品質ゲートが成功する。
- 検証:
  - `python3 scripts/dev.py docs --check`
  - `python3 scripts/dev.py verify`

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| enumをstructへ置換して旧saveを壊す | B1実機saveをロードできない | 旧variantを維持し、新variantだけを追加する |
| 複数許可を混載許可と誤解する | 同一セルへ異種予約が入る | stored/reserved resource一致の既存evaluatorを変更しない |
| 0件許可が `Any` と混同される | 全解除しても搬入が続く | empty setを明示variantで保持し、domain testを追加する |
| UIが縦に伸びて既存操作が画面外になる | target/priority/exportを操作できない | 2列wrap、viewport比の最大高、縦scroll、1280x720・最大scaleの自動高さ予算と実機受入を追加する |
| 資材追加時に一覧だけ更新漏れする | UIとdomainの集合が不一致になる | 公開定数をdomainとUIで共有し、全資材の集合回帰を固定する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 計画完了時:
  - `cargo test --workspace`
  - `python3 scripts/dev.py docs --check`
  - `git diff --check`
- 手動確認シナリオ:
  - 管理Stockpileを選択すると9資材と全選択・全解除が見え、cycle操作がない。
  - 全解除で `Disabled` となることと、WoodとRockだけをONにして両方の空セルへの搬入候補化、
    Boneの拒否を確認する。
  - 1280x720、UI Scale 1.25で縦scrollし、Target、Priority、Export、範囲適用へ到達する。
  - Wood在庫セルでWoodをOFFにするとDrainingとなり、再度ONにすると通常状態へ戻る。
  - 複数選択を矩形へコピーし、F5/F9後もチェック状態が一致する。
- パフォーマンス確認（必要時）:
  - Info Panelは静的9行を再利用し、毎frameのentity生成・破棄を行わない。

## 8. ロールバック方針

- どの単位で戻せるか: domain/save集合、UI一覧、文書・受入項目を同一変更単位として戻す。
- 戻す時の手順: 新variantを生成しないUIへ戻した後、旧 `Any / Only` のdeserialize回帰を確認する。新variantを保存済みのfileは旧コードで読めないため、部分的な型だけの巻き戻しは行わない。

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: `M1`、`M2`、`M3`
- 未着手/進行中: なし

### 次のAIが最初にやること

1. 本計画に残作業はない。チェックリスト実装とB1-R14〜R15の実機受入記録としてarchive状態を維持する。
2. B1全体の完了記録は
   `docs/plans/archive/stockpile-policy-manual-acceptance-plan-2026-07-23.md` を参照する。

### ブロッカー/注意点

- B1-R05は`2026-07-25`に実機再受入まで完了しており、B1に残るblockerはない。
- `docs/README.md`、`docs/plans/README.md`、提案書にはHelp/A2の別作業差分があるため上書きしない。

### 参照必須ファイル

- `crates/hw_logistics/src/zone.rs`
- `crates/hw_logistics/src/stockpile_policy.rs`
- `crates/hw_ui/src/panels/info_panel/`
- `crates/bevy_app/src/systems/save/schema.rs`
- `docs/logistics.md`
- `docs/info_panel_ui.md`
- `docs/plans/archive/stockpile-policy-manual-acceptance-plan-2026-07-23.md`

### 最終確認ログ

- 最終 `cargo check --workspace`: `2026-07-24 / pass`（`scripts/dev.py verify` 内）
- 最終 `cargo clippy --workspace --all-targets -- -D warnings`: `2026-07-24 / pass`（0 warnings）
- 最終 `cargo test --workspace`: `2026-07-24 / pass`（`scripts/dev.py verify` 内）
- focused test: `2026-07-24 / pass`（`hw_logistics 62`、`hw_ui 49`、`bevy_app 238`）
- checklist実機受入: `2026-07-24 / pass / B1-R14〜B1-R15`
- B1-R05実機再受入: `2026-07-25 / pass`
- 未解決エラー: なし

### Definition of Done

- [x] 目的に対応するマイルストーンが全て完了
- [x] 影響ドキュメントが更新済み
- [x] `cargo check --workspace` が成功
- [x] `cargo clippy --workspace --all-targets -- -D warnings` が成功
- [x] `cargo test --workspace` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-07-24` | `Codex` | 単一cycleを複数資材チェックリストへ置換する設計、save互換、受入条件を固定 |
| `2026-07-24` | `Codex` | 複数許可集合、旧variant互換、静的2列UI、矩形patch、save/物流回帰、恒久docsを実装 |
| `2026-07-24` | `Codex` | 自己レビューで最大UI scaleの縦overflow、旧RON固定fixture、実button binding、空集合state、表示名を補強 |
| `2026-07-24` | `Codex` | workspace全品質ゲートとdocs検証を完了し、B1-R14〜R15を実機受入計画へ引き継いでarchive |
| `2026-07-25` | `User / Codex` | B1-R14〜R15とB1-R05の実機受入完了を反映し、B1に残るblockerがないことを記録 |
