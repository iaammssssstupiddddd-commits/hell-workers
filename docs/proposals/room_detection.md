# Room検出機能

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `room-detection-proposal-2026-02-23` |
| ステータス | `Accepted` |
| 作成日 | `2026-02-23` |
| 最終更新日 | `2026-02-23` |
| 作成者 | `AI` |
| 関連計画 | `docs/plans/room-detection-plan-2026-02-23.md` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: 壁・ドア・床は個別のエンティティとして管理されており、囲まれた空間を論理的に認識する仕組みがない
- 問題: 将来のRoom系ゲームプレイ機能（温度、モラル、部屋品質バフなど）の基盤がない
- なぜ今やるか: ドア機能が実装され、壁・ドア・床の3要素が揃ったため、Room検出の前提条件が整った

## 2. 目的（Goals）

- 完成壁・ドア（1つ以上）・全面床で囲まれたエリアを「Room」として自動検出する
- 検出されたRoomを半透明オーバーレイで視覚的に表示する
- 将来のRoom系機能（温度、モラル、部屋タイプ判定等）の基盤となるデータ構造を提供する

## 3. 非目的（Non-Goals）

- Roomへのバフ/デバフ適用（将来の拡張）
- Roomタイプ（寝室、作業場等）の自動判定（将来の拡張）
- Room内部の家具/設備の管理（将来の拡張）
- UI上でのRoom名称編集

## 4. 提案内容（概要）

- 一言要約: 完成壁・ドア・全面床で囲まれた領域をFlood-fillで検出し、Room entityとして管理する
- 主要な変更点:
  - `src/systems/room/` モジュール新設（検出・バリデーション・ビジュアル）
  - Building変更時のdirtyマーキング → 0.5秒間隔でFlood-fill検出
  - Room entityに半透明カラーオーバーレイを子スプライトとして生成
- 期待される効果: プレイヤーが建設した囲いが「部屋」として認識され、視覚的にフィードバックされる

## 5. 詳細設計

### 5.1 仕様

**Roomの定義:**
連続する床タイルの集合 `T` であり、以下を全て満たすもの:
- `T` 内の全タイルに完成 `Building { kind: Floor }` が存在する
- `T` の全4近傍タイルのうち `T` に含まれないものは、全て以下のいずれか:
  - 完成壁: `Building { kind: Wall, is_provisional: false }`
  - ドア: `Building { kind: Door }`
- 境界に少なくとも1つのドアが存在する
- `|T| <= ROOM_MAX_TILES (400)`

**検出トリガー:**
- `Building` コンポーネントの追加・変更・削除時に周辺をdirtyとしてマーク
- 0.5秒間隔のクールダウンで検出システムが実行

**検出アルゴリズム (Flood-fill):**
1. dirty領域内の床タイルを seed として収集
2. 各 seed から4方向にフラッドフィル:
   - 完成壁 → 境界記録、展開しない
   - ドア → 境界記録、展開しない
   - 完成床 → 内部タイル記録、4近傍をキューに追加
   - それ以外（空、仮壁、Blueprint、他建物） → Room不成立
   - 範囲外 → Room不成立
   - タイル数超過 → Room不成立
3. 境界ドア >= 1 かつ 内部タイル >= 1 なら Room 成立

**バリデーション:**
- 2秒ごとに既存Roomを検証
- 壁/ドア/床エンティティの存在・状態を確認
- 不正なRoomは dirty にマークして再検出

**例外ケース:**

| ケース | 挙動 |
|--------|------|
| L字型/不規則形状 | Flood-fillが自然に検出 |
| 壁を共有する隣接Room | 共有壁は両Room の boundary に含まれる |
| 入れ子Room | 内壁で区切られ、別Roomとして検出 |
| ドアのみで仕切り | ドアは境界なので別Room |
| 仮壁 (provisional) | 境界として認識しない → Room不成立 |
| 床が1タイル欠け | Flood-fillが漏れる → Room不成立 |
| 建設中 (Blueprint) | 境界/床にならない |
| 壁破壊 | validation → dirty → 再検出 → Room消滅 |
| 400タイル超 | Room不成立 |
| 内部にTank/RestArea等 | 床を上書きするためRoom不成立 |

**既存仕様との整合:** 既存の建築システムに変更なし。Room検出は読み取り専用で WorldMap と Building コンポーネントを参照するのみ。

### 5.2 変更対象（想定）

**新規作成:**
- `src/systems/room/mod.rs` — RoomPlugin
- `src/systems/room/components.rs` — Room, RoomBounds
- `src/systems/room/detection.rs` — 検出アルゴリズム, RoomDetectionState, RoomTileLookup
- `src/systems/room/dirty_mark.rs` — Building変更監視
- `src/systems/room/validation.rs` — 既存Room検証
- `src/systems/room/visual.rs` — オーバーレイ生成

**変更:**
- `src/systems/mod.rs` — `pub mod room;` 追加
- `src/constants/building.rs` — Room定数追加
- `src/constants/render.rs` — Z_ROOM_OVERLAY 追加
- `src/plugins/logic.rs` — Room検出システム登録
- `src/plugins/visual.rs` — Roomオーバーレイシステム登録

### 5.3 データ/コンポーネント/API 変更

**追加:**
- `Room` コンポーネント: tiles, wall_entities, door_entities, bounds, tile_count
- `RoomBounds`: min_x, min_y, max_x, max_y
- `RoomDetectionState` リソース: dirty_positions, cooldown
- `RoomTileLookup` リソース: tile_to_room 逆引き
- `RoomTileOverlay` マーカーコンポーネント
- `ROOM_MAX_TILES`, `ROOM_DETECTION_COOLDOWN_SECS`, `Z_ROOM_OVERLAY` 定数

**変更:** なし
**削除:** なし

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| Flood-fill検出 + dirty tracking | 採用 | イベント駆動で効率的、O(400)上限で計算量も安全 |
| 毎フレーム全マップスキャン | 不採用 | 100x100マップに対して無駄が大きい |
| 壁エッジトレース方式 | 不採用 | L字型や複雑形状の処理が煩雑、Flood-fillの方がシンプル |

## 7. 影響範囲

- ゲーム挙動: Room検出とオーバーレイ表示のみ。既存のゲームプレイには影響なし
- パフォーマンス: Flood-fillは最大400タイル × dirty seed数。0.5秒クールダウンで制限。影響軽微
- UI/UX: 完成Roomに半透明カラーオーバーレイが表示される
- セーブ互換: 影響なし（Room entityは毎回再検出される一時的なもの）
- 既存ドキュメント更新: `docs/building.md` にRoom検出の記述を追加

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 大量のRoom/壁変更でCPU負荷 | フレームドロップ | ROOM_MAX_TILES上限 + クールダウン制御 |
| Room entityのダングリング参照 | パニック | validation systemで定期的にチェック、Optionで安全にアクセス |
| Building削除時の位置情報欠落 | Room再検出漏れ | Observer + 既存Room境界エンティティ監視の二重保護 |

## 9. 検証計画

- `cargo check` でコンパイルエラーなし
- 手動確認シナリオ:
  1. 壁4面 + ドア1面で5x5エリアを囲い、全面に床を敷く → Room検出、オーバーレイ表示
  2. 壁1枚を破壊 → Room消滅
  3. 仮壁のみで囲む → Room不成立
  4. 床を1タイル欠かす → Room不成立
  5. L字型の部屋を建設 → 正しく検出
  6. 2部屋を壁共有で隣接配置 → 別Roomとして検出

## 10. ロールアウト/ロールバック

- 導入手順: `src/systems/room/` モジュール追加 → プラグイン登録
- 段階導入の有無: Phase 1（検出+表示のみ）で完結。将来Phase 2でバフ等を追加可能
- 問題発生時の戻し方: `RoomPlugin` の登録を外すだけで無効化可能

## 11. 未解決事項（Open Questions）

- [x] 仮壁の扱い → 完成壁のみ
- [x] 床の要件 → 全面床が必要
- [x] 内部家具の扱い → Phase 1では考慮しない
- [x] サイズ制限 → 400タイル上限

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`
- 直近で完了したこと: 提案書作成
- 現在のブランチ/前提: master

### 次のAIが最初にやること

1. `src/systems/room/` ディレクトリ作成
2. components.rs でRoom, RoomBounds定義
3. detection.rs でFlood-fillアルゴリズム実装

### ブロッカー/注意点

- `WorldMap.buildings` は1グリッドに1エンティティのみ。Floor上にTank等を置くとFloorが上書きされる
- Bevy 0.18 の Observer API を使用する場合、ソースコードで確認すること

### 参照必須ファイル

- `src/world/map/mod.rs` — WorldMap, grid座標変換
- `src/systems/jobs/mod.rs` — Building, BuildingType, Blueprint
- `src/systems/jobs/door.rs` — Door, DoorState
- `src/plugins/logic.rs` — Logic systemの登録順序
- `src/plugins/visual.rs` — Visual systemの登録
- `src/constants/render.rs` — Zレイヤー定数
- `src/constants/building.rs` — 建物関連定数

### 完了条件（Definition of Done）

- [x] 提案内容がレビュー可能な粒度で記述されている
- [x] リスク・影響範囲・検証計画が埋まっている
- [x] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-23` | `AI` | 初版作成 |
| `2026-02-23` | `Codex` | ステータスと DoD の整合を更新 |
