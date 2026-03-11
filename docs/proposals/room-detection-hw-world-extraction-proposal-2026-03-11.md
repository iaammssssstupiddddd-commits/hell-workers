# Room Detection Core `hw_world` 抽出提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `room-detection-hw-world-extraction-proposal-2026-03-11` |
| ステータス | `Draft` |
| 作成日 | `2026-03-11` |
| 最終更新日 | `2026-03-11` |
| 作成者 | `Codex` |
| 関連計画 | `docs/plans/archive/room-detection-hw-world-extraction-plan-2026-03-11.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状:
  `src/systems/room/detection.rs` には、部屋候補の flood fill、境界判定、妥当性判定、ECS entity の再生成が同居している。
- 問題:
  room detection の計算コアは world/grid ドメインの純ロジックに近いが、root 側 ECS shell と同じモジュールにあるため、`hw_world` に置ける部分と root に残すべき部分が分離されていない。
- なぜ今やるか:
  room detection は world responsibilities に自然に属し、`hw_world` に寄せる候補として境界が比較的明瞭である。root 側に残すのは room entity の spawn/despawn と resource 更新だけでよい。

## 2. 目的（Goals）

- room detection の純粋な探索・判定コアを `hw_world` に寄せる。
- root 側 `detect_rooms_system` を thin adapter に縮小する。
- room 周りの責務を world/grid ドメインとして整理する。

## 3. 非目的（Non-Goals）

- room overlay visual の同時移設。
- `Room`, `RoomBounds`, `RoomTileLookup` を今回中に完全 crate 共有化すること。
- room validation / dirty mark の全面再設計。
- save format やゲーム挙動の変更。

## 4. 提案内容（概要）

- 一言要約:
  room detection の flood fill と候補判定を `hw_world` へ抽出し、root 側には ECS 反映だけを残す。
- 主要な変更点:
  - `build_detection_input`, `room_is_valid_against_input`, `detect_rooms`, `flood_fill_room` を `hw_world` 側へ寄せる。
  - `RoomCandidate` 相当の純データ構造を `hw_world` 側に置く。
  - root 側 system は `q_buildings` と `WorldMapRead` から入力を作り、`hw_world` の結果を room entity / lookup resource に反映するだけにする。
- 期待される効果:
  - world ロジックと ECS wiring の責務が分離される。
  - room detection のテスト対象を pure function に寄せやすくなる。
  - `hw_world` のドメイン範囲が docs 上の責務と揃う。

## 5. 詳細設計

### 5.1 仕様

- 振る舞い:
  - 床・壁・ドアから room 候補を構築するロジックは変更しない。
  - `ROOM_MAX_TILES` や map 境界判定の扱いも維持する。
- 例外ケース:
  - room entity の spawn/despawn、`RoomTileLookup` の再構築は root shell に残す。
  - `Room` / `RoomBounds` が ECS component のままなら、`hw_world` 側では同型または変換用データ構造を用意する。
- 既存仕様との整合:
  - `docs/room_detection.md` の room 判定仕様を維持する。
  - `docs/cargo_workspace.md` の `hw_world` 責務と矛盾しない。

### 5.2 変更対象（想定）

- `src/systems/room/detection.rs`
- `src/systems/room/components.rs`
- `src/systems/room/resources.rs`
- `crates/hw_world/src/`
- `docs/cargo_workspace.md`
- `docs/architecture.md`
- `docs/room_detection.md`

### 5.3 データ/コンポーネント/API 変更

- 追加:
  - `hw_world` 側の room detection core module
  - pure input / candidate type
- 変更:
  - root `detect_rooms_system` の役割を adapter に限定
  - 必要なら `Room` 生成前の変換レイヤーを追加
- 削除:
  - root 側にある純粋探索ロジックの重複

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| room detection core だけを `hw_world` に移す | 採用 | world ドメインとして自然で、ECS shell と切り分けやすい。 |
| room system 全体を一気に `hw_world` へ移す | 不採用 | `Room` / `RoomTileLookup` が root ECS モデルで、段階分離が必要。 |
| 現状維持 | 不採用 | world ロジックが root に残り続け、責務が曖昧なままになる。 |

## 7. 影響範囲

- ゲーム挙動:
  原則変更しない。room 判定結果と overlay 表示条件を維持する。
- パフォーマンス:
  直接改善よりも、純ロジック化により見通しと検証性が上がる。
- UI/UX:
  直接影響なし。
- セーブ互換:
  なし。
- 既存ドキュメント更新:
  `docs/room_detection.md`, `docs/architecture.md`, `docs/cargo_workspace.md`。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| `Room` / `RoomBounds` / `RoomTileLookup` が root component/resource のまま | system 全体は移せない | まず detection core のみを pure module 化する |
| pure data と ECS component の二重定義が発生する | 型変換コストと可読性低下 | 候補構造は最小限にし、component への変換を root adapter に限定する |
| detection と validation の責務分離が曖昧になる | 追加変更で再び root に戻る | `detect` と `apply` を文書化し、モジュール境界を固定する |
| overlay visual が room data に強く結合している | 後続の visual 整理が難しくなる | 今回は visual を out-of-scope と明記する |

## 9. 検証計画

- `cargo check --workspace`
- 手動確認シナリオ:
  - 壁・床・ドアで囲まれた部屋が従来通り生成される
  - 壁の欠けた領域が room として認識されない
  - room overlay が更新遅延や欠落なく再生成される
- 計測/ログ確認:
  - dirty tile 更新後の room 再検出タイミングを確認する

## 10. ロールアウト/ロールバック

- 導入手順:
  1. detection input / candidate を pure data に切り出す。
  2. flood fill と判定を `hw_world` に移す。
  3. root 側は candidate を ECS room entity に反映する shell に縮小する。
- 段階導入の有無:
  あり。core 抽出と model 移設を分ける。
- 問題発生時の戻し方:
  detection core の import 先を root 実装へ戻し、変換層だけを撤回する。

## 11. 未解決事項（Open Questions）

- [ ] `Room` / `RoomBounds` を `hw_world` 所有に寄せるべきか
- [ ] `RoomTileLookup` を root resource のままにするか shared model 化するか
- [ ] `validation.rs` を同時に `hw_world` 寄せする価値があるか

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 直近で完了したこと:
  - room detection を crate 化候補として棚卸しした
- 現在のブランチ/前提:
  - room detection core と ECS 反映が同じ root module にある

### 次のAIが最初にやること

1. `detection.rs` で pure logic と ECS apply を分割する境界を一覧化する。
2. `RoomCandidate` 相当の shared data shape を決める。
3. 実装計画を `docs/plans/` に起こす。

### ブロッカー/注意点

- `Room` / `RoomBounds` / `RoomTileLookup` は root 側 ECS model として使われている。
- visual と validation を一度に巻き込むと proposal のスコープが崩れる。
- map 境界と `ROOM_MAX_TILES` の扱いは回帰しやすい。

### 参照必須ファイル

- `docs/cargo_workspace.md`
- `docs/architecture.md`
- `docs/room_detection.md`
- `src/systems/room/detection.rs`
- `src/systems/room/components.rs`
- `src/systems/room/resources.rs`

### 完了条件（Definition of Done）

- [ ] 提案内容がレビュー可能な粒度で記述されている
- [ ] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-03-11` | `Codex` | 初版作成 |
