# crate boundary alignment plan

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `crate-boundary-alignment-plan-2026-03-18` |
| ステータス | `Complete` |
| 作成日 | `2026-03-18` |
| 最終更新日 | `2026-03-18` |
| 作成者 | `AI (Codex)` |
| 関連提案 | `N/A` |
| 関連Issue/PR | `N/A` |

## 1. 目的

- 解決したい課題: `docs/crate-boundaries.md` の規約と、現在の workspace 実装・運用が一部ずれている。
- 到達したい状態: 「緩めるべき規約」と「実際にコードを leaf crate へ寄せるべき箇所」を分離し、以後の判断基準を一貫させる。
- 成功指標:
  - `docs/crate-boundaries.md` / `docs/cargo_workspace.md` / 関連 README が同じ判断基準を指す
  - root 登録を許容するケースと、root 実装を禁止するケースが文章で明確になる
  - room / construction まわりの root 残留ロジックのうち、移設対象が具体的なマイルストーンとして定義される

## 2. スコープ

### 対象（In Scope）

- crate boundary ルールのうち、system 実装場所と plugin 登録場所の基準整理
- `bevy_app` に残っている room / construction 関連ロジックの棚卸し
- 文書更新と、それに追随する最小限のコード移設計画

### 非対象（Out of Scope）

- 新規 crate の追加
- `GameAssets` / `app_contexts` / `NextState<PlayMode>` を伴う root adapter の全面再設計
- 一度の変更で全 root 残留 system を移設すること
- UI / selection / startup 系の boundary 再設計

## 3. 現状とギャップ

- 現状:
  - `docs/crate-boundaries.md` は「self-contained system は leaf plugin が登録する」前提で読める。
  - 実コードでは `bevy_app` の scheduling facade から leaf-owned system / observer を直接登録している箇所がある。
  - 一方で、root 固有依存を持たない room dirty tracking / overlay sync / construction phase transition が `bevy_app` に残っている。
  - **【確認済み】** `Room` / `RoomOverlayTile` は既に `hw_world::room_detection` に定義されており、`bevy_app` は re-export しているだけ。`Building` / `Door` も `hw_jobs` 所有で、`hw_world/src/room_systems.rs` は既に `hw_jobs::Building` を直接参照している。すなわち dirty_mark / overlay sync の系はコンポーネント型移設が不要で、システム関数の移設のみで完結する。
- 問題:
  - 規約どおりに厳格解釈すると、実装のかなりの部分が違反に見える。
  - 逆に実装をそのまま正当化すると、root shell の境界が曖昧になり、何でも `bevy_app` に残せてしまう。
- 本計画で埋めるギャップ:
  - 「コード所有」と「system 登録」を分離して記述する。
  - root 側に残してよいのは root-only resource / adapter / facade に限定する。
  - それに当てはまらない実装は、移設候補として文書とコードをそろえる。

## 4. 実装方針（高レベル）

- 方針:
  - まず規約を 2 軸で書き直す。
  - 1 軸目は「実装本体の所有先」。
  - 2 軸目は「plugin / observer の登録元」。
  - 登録元は root facade を許容するが、実装本体が root-only 依存を持たない場合は leaf 所有を原則にする。
- 設計上の前提:
  - root での登録は ordering 集約のために必要な場合がある。
  - ただし root に実装本体を残してよい理由にはならない。
  - 「唯一の登録元」を維持し、二重登録は引き続き禁止する。
- Bevy 0.18 APIでの注意点:
  - system / observer の二重登録は schedule 初期化や ordering 解決を壊すため避ける。
  - 移設時は `Plugin::build` と root 側 `add_systems` / `add_observer` の両方を同時に確認する。
  - system function の import path を変えても system identity と ordering 参照が壊れないか確認する。

## 5. マイルストーン

## M1: registration ルールを現実に合わせて明文化する

- 変更内容:
  - `docs/crate-boundaries.md` に「system 実装の所有先」と「登録責務」を分けた節を追加する。
  - root から leaf-owned system を登録してよい条件を明記する。
  - 例外条件を「root-only resource / app_context / visual handle inject / facade ordering」に限定する。
- 変更ファイル:
  - `docs/crate-boundaries.md`
  - `docs/cargo_workspace.md`
  - `crates/bevy_app/src/README.md`
- 完了条件:
  - [ ] leaf-owned / root-registered のパターンが文書上で正当化される
  - [ ] 「登録元は一箇所だけ」の規約が明記される
  - [ ] `hw_jobs::visual_sync` / `hw_logistics::visual_sync` のような現行パターンが規約と矛盾しなくなる
- 検証:
  - 文書レビュー

## M2: root に残すべきでない実装を固定する

- 変更内容:
  - room dirty tracking / overlay sync を `hw_world` 所有へ寄せる計画を具体化する。
  - construction phase transition のうち root-only 依存がない部分を抽出し、`hw_jobs` か適切な leaf crate へ寄せる方針を確定する。
  - 混在ファイルは「root 残留パート」と「移設パート」に分割する。
- 変更ファイル:
  - `crates/bevy_app/src/systems/room/dirty_mark.rs`
  - `crates/bevy_app/src/systems/room/visual.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/phase_transition.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/phase_transition.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/completion.rs`
  - `docs/cargo_workspace.md`
  - `crates/hw_world/README.md`
  - `crates/hw_jobs/README.md`
- 完了条件:
  - [ ] room dirty tracking / overlay が leaf 移設対象として明文化される（コンポーネント型は既に hw_world / hw_jobs 所有であるため型移設は不要と明記する）
  - [ ] `dirty_mark.rs` の `Building` / `Door` が `hw_jobs` re-export であることを確認し、hw_world への直接移設が可能であると文書化する
  - [x] `floor_construction_phase_transition_system` が依存する `construction_shared::remove_tile_task_components` の移設要否を確認済み: `hw_jobs::model::remove_tile_task_components` として既に hw_jobs 所有。`bevy_app/systems/jobs/construction_shared.rs` は `pub use hw_jobs::remove_tile_task_components;` の thin re-export に過ぎない。型移設不要・確認完了。
  - [ ] wall / floor construction で「root に残す関数」と「leaf へ移す関数」の境界が定義される（具体例: `wall_construction/phase_transition.rs` には `wall_framed_tile_spawn_system`（`Building3dHandles` = root-only 依存あり → **bevy_app 残留**）と `wall_construction_phase_transition_system`（root-only 依存なし → **hw_jobs 移設対象**）が同居している）
  - [ ] mixed-responsibility file の分割方針が文書化される
- 検証:
  - `cargo check --workspace`

## M3: 第1弾のコード移設を実施する

- 変更内容:
  - room dirty tracking / overlay sync を `hw_world` へ移す。
  - wall / floor construction phase transition の純粋 apply 部分を leaf 側へ移す。
  - root 側は thin shell または facade 登録だけに縮退する。
- 変更ファイル:
  - `crates/hw_world/src/room_systems.rs`（既存ファイルへの追記。dirty_mark / overlay sync 関数を追加する）
  - `crates/hw_world/src/lib.rs`
  - `crates/bevy_app/src/systems/room/*.rs`
  - `crates/hw_jobs/src/lib.rs`
  - `crates/hw_jobs/src/construction.rs`
  - `crates/bevy_app/src/systems/jobs/floor_construction/*.rs`
  - `crates/bevy_app/src/systems/jobs/wall_construction/*.rs`
  - `crates/bevy_app/src/plugins/logic.rs`
  - `docs/crate-boundaries.md`
  - `docs/cargo_workspace.md`
- 完了条件:
  - [ ] room 系 root 実装が shell / wiring に縮退する
  - [ ] construction の純粋 phase transition が root から消える
  - [ ] root 側に残る理由がある関数だけが `bevy_app` に残る（M2 で決定した境界に一致すること）
  - [ ] `sync_room_overlay_tiles_system` 移設後も `GameSystemSet::Visual` の ordering が維持されていること（root 側の登録位置を確認する）
- 検証:
  - `cargo check --workspace`

## M4: 例外リストと残留 backlog を固定する

- 変更内容:
  - 今回移さない root 残留ロジックを「許容理由つき」で列挙する。
  - 後続候補を backlog 化し、同じ議論を繰り返さないようにする。
- 変更ファイル:
  - `docs/crate-boundaries.md`
  - `docs/cargo_workspace.md`
  - `docs/architecture.md`
  - `docs/plans/README.md`
- 完了条件:
  - [x] `wall_framed_tile_spawn_system` の bevy_app 残留理由を文書化（`Building3dHandles` が bevy_app 固有型のため移設不可）
  - [x] 次に見るべき boundary 候補が backlog として整理される
  - [x] 例外の条件がファイル単位ではなく責務単位で説明される
- 検証:
  - 文書レビュー

### bevy_app 残留が許容される関数（理由付き）

| 関数 | ファイル | 残留理由 |
| --- | --- | --- |
| `wall_framed_tile_spawn_system` | `bevy_app/src/systems/jobs/wall_construction/phase_transition.rs` | `Building3dHandles`（bevy_app 専用リソース）に依存。Leaf 移設には `Building3dHandles` の抽象化 (`WallVisualHandles` パターン適用) が前提となる。 |
| 各種 visual spawn / completion 系 (`wall_framed_tile_spawn_system` 以外も同様) | `bevy_app/src/systems/jobs/` 配下 | `GameAssets` / `Building3dHandles` への依存あり |

### 残留 backlog（後続候補）

- `wall_framed_tile_spawn_system` の hw_jobs 移設：`Building3dHandles` を `WallVisualHandles` 注入パターンへ置換すれば可能。
- `floor_construction_completion_system` / `wall_construction_completion_system` の依存調査・移設検討。
- `building_completion/` 系の root-only 依存整理。

## 6. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 規約を緩めすぎて root shell の境界が崩れる | 今後の実装が再び `bevy_app` に集まりやすくなる | 緩める対象を「登録責務」に限定し、「実装所有」はむしろ厳格化する |
| `hw_jobs` に system を増やすことで README / `_rules.md` と齟齬が出る | 新しい違反を増やす | M2 で `hw_jobs` の責務説明も同時に更新する |
| room 実装移設で observer / overlay の ordering が壊れる | 再検出や表示更新の回帰が出る | 移設前後で root plugin の登録箇所と chain を比較し、唯一の登録元を維持する |
| mixed-responsibility file を一気に動かして差分が大きくなる | レビュー負荷が高くなる | root-only 部分と pure apply 部分を先に分離してから移設する |

## 7. 検証計画

- 必須:
  - `cargo check --workspace`
- 手動確認シナリオ:
  - wall / floor construction が phase transition すること
  - room 検出が建物追加・削除・移動で再計算されること
  - visual mirror 更新が従来どおり動くこと
- パフォーマンス確認（必要時）:
  - room overlay 再生成時の明らかな退行がないことを確認する

## 8. ロールバック方針

- どの単位で戻せるか:
  - M1 は docs のみなのでコミット単位で戻せる
  - M2 / M3 は room 系と construction 系を別コミットに分けて戻せる
- 戻す時の手順:
  - docs だけの変更は該当コミットを revert する
  - code migration は crate ごとに revert し、root shell に戻す

## 9. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 完了済みマイルストーン: M1, M2, M3, M4
- 未着手/進行中: なし

### 次のAIが最初にやること

すべてのマイルストーンは完了済み。この計画書はアーカイブ候補。

### ブロッカー/注意点

- `hw_jobs` は現状 README では「型・状態機械のみ」と説明されているため、system 移設時は README / `_rules.md` 更新が必須。
- `wall_framed_tile_spawn_system` や building completion は `Building3dHandles` / `GameAssets` 依存のため、phase transition と同列に扱わない。
- root 登録を残す場合でも、同じ system を leaf plugin と二重登録しない。

### 参照必須ファイル

- `docs/crate-boundaries.md`
- `docs/cargo_workspace.md`
- `crates/bevy_app/src/plugins/logic.rs`
- `crates/hw_jobs/src/visual_sync.rs`
- `crates/hw_logistics/src/visual_sync.rs`
- `crates/bevy_app/src/systems/room/dirty_mark.rs`
- `crates/bevy_app/src/systems/room/visual.rs`
- `crates/bevy_app/src/systems/jobs/floor_construction/phase_transition.rs`
- `crates/bevy_app/src/systems/jobs/wall_construction/phase_transition.rs`

### 最終確認ログ

- 最終 `cargo check`: `2026-03-18` / `pass`
- 未解決エラー:
  - なし

### Definition of Done

- [ ] 目的に対応するマイルストーンが全て完了
- [ ] 影響ドキュメントが更新済み
- [ ] `cargo check` が成功

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-18` | `AI (Codex)` | 初版作成 |
| `2026-03-18` | `AI (Copilot)` | レビュー指摘を反映: Room/RoomOverlayTile・Building/Door が既に leaf 所有であることを §3 現状に追記; M2 完了条件に型移設不要の明記・remove_tile_task_components 移設要否の確認項目を追加; M3 変更ファイルに room_systems.rs が既存ファイルへの追記である旨を明記; M3 完了条件に Visual ordering 維持確認を追加 |
| `2026-03-18` | `AI (Copilot)` | 実装完了: docs/crate-boundaries.md §3.3 に Ordering Facade 例外と実装所有/登録元 2 軸原則を追記 (M1); room 系 dirty_mark・visual を hw_world/room_systems.rs へ移設、bevy_app thin re-export 化 (M3); floor/wall phase_transition を hw_jobs/construction.rs へ移設、wall_framed_tile_spawn_system は Building3dHandles 依存のため bevy_app 残留 (M3); M4 例外リスト・backlog を計画書に追記; cargo check --workspace pass |
