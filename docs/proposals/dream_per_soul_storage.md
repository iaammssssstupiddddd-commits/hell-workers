# Dream Per-Soul Storage（soul個別dream貯蔵システム）

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `dream-per-soul-storage-proposal-2026-02-26` |
| ステータス | `Approved` |
| 作成日 | `2026-02-26` |
| 最終更新日 | `2026-02-26` (実装完了) |
| 作成者 | `AI (Claude)` |
| 関連計画 | `TBD` |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

- 現状: Dreamはグローバルプール(`DreamPool.points`)に直接加算される。soulの睡眠レートで`DreamPool`に即時反映され、soul個別のdream貯蔵量という概念がない。
- 問題: dreamが「soulの内部状態」として表現されておらず、soul管理の戦略性が薄い。dreamの蓄積がsoulの精神状態に影響するメカニズムがなく、労働 vs 休息のジレンマが一方向的。
- なぜ今やるか: ストレス・疲労に続くsoul管理の第3軸として、dream圧力を導入することでゲームプレイの深みを増す。

## 2. 目的（Goals）

- soulごとにdream貯蔵量を持たせ、蓄積→放出のサイクルを作る
- dreamが多いsoulほどストレスが溜まりやすくなるメカニズムの導入
- 休息の重要性を高める（dreamを定期的に放出しないとストレスが加速する）

## 3. 非目的（Non-Goals）

- DreamQualityシステムの廃止（ビジュアル用途として維持）
- DreamPool消費先（植林等）の変更
- UIの大幅改修（将来的にsoul個別のdreamバーは検討するが本提案のスコープ外）

## 4. 提案内容（概要）

- 一言要約: dreamをsoul個別の内部ステータス（0.0–100.0）にし、睡眠・休憩時にグローバルDreamPoolへ変換する
- 主要な変更点:
  1. `DamnedSoul`に`dream: f32`フィールド追加（上限100.0）
  2. 非睡眠・非休憩中に行動状態に応じたレートでdreamが蓄積
  3. 睡眠・休憩中にsoulのdreamが徐々にDreamPoolへ放出
  4. dream量に比例してストレス蓄積レートに乗算ペナルティ
  5. 休憩所の固定レートDreamPool加算を廃止、soul dream放出に統一
- 期待される効果: soulを適切に休ませるインセンティブが強まり、労働配分の戦略性が向上

## 5. 詳細設計

### 5.1 仕様

#### 5.1.1 dream蓄積（非睡眠・非休憩時）

起きている間、行動状態に応じたレートでsoulのdreamが増加する。

| 行動状態 | 蓄積レート（/秒） | 備考 |
| :--- | :--- | :--- |
| 労働中（タスク実行） | 高（要調整） | 過酷な労働ほど夢を見たくなる |
| アイドル（Wandering/Sitting） | 低（要調整） | ぼんやりしていても少しずつ蓄積 |
| 集会中（Gathering） | 中（要調整） | 仲間と過ごして刺激を受ける |
| 逃走中（Escaping） | 高（要調整） | 恐怖が夢を加速させる |

- 上限: `100.0`（到達後は蓄積停止、溢れない）
- 初期値: `0.0`（スポーン時）

#### 5.1.2 dream放出（睡眠・休憩中）

睡眠・休憩中にsoulのdreamが徐々にDreamPoolへ移動する。

- 放出レート: 一律固定（DreamQualityによらない）
- 毎フレーム: `drain = min(soul.dream, DREAM_DRAIN_RATE * dt)`
- `soul.dream -= drain`
- `dream_pool.points += drain`
- dream=0になったら放出停止し、**睡眠・休憩を強制終了**する（5.1.7参照）

DreamQualityは放出レートに影響しないが、ビジュアルエフェクト（パーティクル色・形状）には引き続き使用される。

#### 5.1.3 休憩所の変更

- **廃止**: `occupant_count × REST_AREA_DREAM_RATE` のグローバルプール直接加算
- **統一**: 休憩所でも上記5.1.2の放出メカニズムを使用
- 休憩所の利点はバイタル回復速度のボーナスに集中（既存のfatigue/stress回復は維持）
- 休憩所での放出レートを睡眠時より高くするかは調整次第（ボーナス係数の検討余地あり）

#### 5.1.4 ストレス連動

dreamの蓄積量がストレス蓄積レートに乗算影響を与える。

```
effective_stress_rate = base_stress_rate * (1.0 + soul.dream * DREAM_STRESS_MULTIPLIER)
```

- `DREAM_STRESS_MULTIPLIER`: 要調整（例: `0.01` → dream=100で+100%、`0.005` → dream=100で+50%）
- 労働中のストレス蓄積のみに適用するか、全ストレス源に適用するかは調整で決定

#### 5.1.5 dream=0時の睡眠・休憩禁止

dreamが0のsoulは睡眠・休憩に入れず、睡眠・休憩中にdreamが0になったら強制的に起こされる。
dreamは「眠る燃料」として機能し、蓄積なしには休めない。

##### 進入ガード（dream <= 0 で阻止）

以下の3箇所すべてにsoul.dream > 0のチェックを追加する。

| 進入パス | ファイル | 関数 | 変更内容 |
| :--- | :--- | :--- | :--- |
| `IdleBehavior::Sleeping` | `transitions.rs` | `select_next_behavior()` | dream<=0ならSleepingを選択肢から除外（Sitting/Wanderingにフォールバック） |
| `GatheringBehavior::Sleeping` | `transitions.rs` | `random_gathering_behavior()` | dream<=0ならSleepingを除外（Wandering/Standing/Dancingの3択） |
| `IdleBehavior::GoingToRest` | `idle_behavior/mod.rs` | `wants_rest_area`条件 | `soul.dream > 0.0` を追加条件に |

##### 放出中のdream枯渇 → 強制起床

睡眠・休憩中にdreamが0に到達した場合:

- **`IdleBehavior::Sleeping`**: `IdleBehavior::Wandering`に遷移（起床）
- **`GatheringBehavior::Sleeping`**: 次のサブ行動をランダム再選択（Standing/Dancing/Wandering）
- **`IdleBehavior::Resting`（休憩所）**: `IdleBehaviorOperation::LeaveRestArea`を発行して退出

判定は`dream_update_system`内で放出処理の直後に行う。

##### ExhaustedGatheringとの関係

`ExhaustedGathering`自体はブロックしない（疲労が高いsoulが集会に参加すること自体は許可）。ただしGathering中のサブ行動としてSleepingが選ばれることは上記ガードで阻止されるため、dream=0のsoulは集会中にWandering/Standing/Dancingのみ行う。

##### ゲームデザイン上の意図

- soulを働かせてdreamを蓄積させないと休ませることができない
- 「労働→dream蓄積→睡眠/休憩でdream放出→DreamPool獲得」の明確なサイクルを形成
- dreamが0のsoulはストレスや疲労が溜まっていても眠れないため、まず何らかの活動でdreamを蓄積する必要がある
- プレイヤーはsoulのdream残量を見て、休ませるタイミングを判断する戦略性

#### 5.1.6 例外ケース

- **StressBreakdown中**: dream蓄積は継続する（breakdown中は睡眠でも休憩でもないため）
- **Drifting（漂流中）**: 蓄積は継続（脱走中も夢は溜まる）
- **NightTerror**: 放出レートは一律のため、NightTerror中もdreamはDreamPoolに変換される（ただしビジュアルは悪夢演出）
- **dream=0 + 高疲労/高ストレス**: 睡眠・休憩できないため、Wandering/Sitting/集会（Dancing/Standing）で過ごす。疲労・ストレス回復は遅いが、行動を続ける間にdreamが蓄積され、再び休めるようになる。

#### 5.1.7 既存仕様との整合

- `dream_update_system`: DreamPool直接加算を削除→soul.dreamからの放出に変更
- `rest_area_update_system`: DreamPool加算ロジックを削除→soul.dream放出に統一
- `DreamQuality`判定ロジック: 変更なし（ビジュアル用途として維持）
- `DreamPool`消費（植林等）: 変更なし
- DreamビジュアルUI（パーティクル、ポップアップ）: 放出元がsoul.dreamになるだけで、DreamPool側の演出は基本維持

### 5.2 変更対象（想定）

- `src/entities/damned_soul/mod.rs` — `DamnedSoul`にdreamフィールド追加
- `src/systems/soul_ai/update/dream_update.rs` — 蓄積・放出ロジック書き換え
- `src/systems/soul_ai/update/rest_area_update.rs` — DreamPool直接加算を削除
- `src/systems/soul_ai/update/vitals_update.rs` — ストレス乗算の適用
- `src/constants/dream.rs` — 新定数追加（蓄積レート、放出レート、ストレス乗算係数）
- `src/systems/soul_ai/decide/idle_behavior/transitions.rs` — sleep選択時のdreamガード追加
- `src/systems/soul_ai/decide/idle_behavior/mod.rs` — wants_rest_area条件にdreamガード追加
- `src/constants/ai.rs` — `REST_AREA_DREAM_RATE` の扱い変更
- `docs/dream.md` — ドキュメント更新

### 5.3 データ/コンポーネント/API 変更

- **変更**: `DamnedSoul` に `dream: f32` フィールド追加（default: 0.0）
- **追加定数**:
  - `DREAM_ACCUMULATE_RATE_WORKING`: 労働中の蓄積レート
  - `DREAM_ACCUMULATE_RATE_IDLE`: アイドル時の蓄積レート
  - `DREAM_ACCUMULATE_RATE_GATHERING`: 集会中の蓄積レート
  - `DREAM_ACCUMULATE_RATE_ESCAPING`: 逃走中の蓄積レート
  - `DREAM_DRAIN_RATE`: 睡眠・休憩中の放出レート
  - `DREAM_MAX`: dream上限値（100.0）
  - `DREAM_STRESS_MULTIPLIER`: ストレス乗算係数
- **削除**: `REST_AREA_DREAM_RATE` の DreamPool 直接加算用途（定数自体は休憩所放出ボーナスとして再利用可能）

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| A: soul個別蓄積＋放出（本提案） | 採用 | dreamがsoul管理の意思決定に直結し、ゲームプレイの深みが増す |
| B: グローバルプール維持＋ストレス連動のみ追加 | 不採用 | soul個別の管理が生まれず、戦略性が限定的 |
| C: dreamを別コンポーネントに分離 | 不採用 | fatigue/stress/motivationと同列の内部ステータスであり、DamnedSoulに統合するのが自然 |

## 7. 影響範囲

- ゲーム挙動: dreamの蓄積・放出サイクルにより、soul管理の優先度判断が変わる。長時間労働させるとdreamが溜まりストレスが加速する。
- パフォーマンス: per-soul計算が追加されるが、既存のvitals更新と同程度の計算量であり影響は軽微。
- UI/UX: 即時的なUI変更は不要。将来的にsoul詳細パネルへのdreamバー表示を検討。DreamPool UIは変更なし。
- セーブ互換: `DamnedSoul`にフィールド追加。Reflectで対応可能（デフォルト0.0）。
- 既存ドキュメント更新: `docs/dream.md`の全面改訂が必要。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| バランス調整が困難 | ストレス乗算が強すぎるとbreakdownが多発 | 定数を分離し、段階的に調整可能にする |
| DreamPool収入が大幅に変わる | 植林コストとのバランスが崩れる | 移行時に蓄積レート・放出レートを現行のDreamPool収入と同等になるよう調整 |
| 休憩所のDream生成ボーナス消失 | 休憩所の価値が下がる | 休憩所での放出レートにボーナス係数を設ける余地を残す |

## 9. 検証計画

- `cargo check` エラーなし
- 手動確認シナリオ:
  - soulが労働中にdreamが増加することを確認（Bevy Inspector等）
  - soulが睡眠中にdreamが減少し、DreamPoolが増加することを確認
  - dream高蓄積soulのストレス蓄積速度が上昇することを確認
  - 休憩所でdreamが放出されることを確認
  - dream=100到達後に蓄積が停止することを確認
  - dream=0のsoulがSleeping/Restingに遷移しないことを確認
  - 睡眠中にdream=0到達でWanderingに遷移することを確認
  - 休憩所滞在中にdream=0到達で退出することを確認
  - dream=0 + 高疲労のsoulがExhaustedGatheringでSleeping以外のサブ行動を行うことを確認
  - 植林のDreamPool消費が正常に動作することを確認

## 10. ロールアウト/ロールバック

- 導入手順:
  1. `DamnedSoul`にdreamフィールド追加
  2. dream蓄積システム実装（vitals更新に統合）
  3. dream放出ロジック実装（dream_update_system改修）
  4. 休憩所のDreamPool直接加算を削除
  5. ストレス乗算の適用
  6. 定数調整・テスト
  7. ドキュメント更新
- 段階導入の有無: 蓄積→放出→ストレス連動の順で段階的に追加可能
- 問題発生時の戻し方: dreamフィールドを無視し、旧ロジックに戻す（蓄積・放出をコメントアウト）

## 11. 未解決事項（Open Questions）

- [ ] 各行動状態の蓄積レート具体値（バランステストで決定）
- [ ] 放出レートの具体値（現行のDreamPool収入と均衡するよう調整）
- [ ] ストレス乗算係数の具体値（breakdownとの閾値バランス）
- [ ] 休憩所での放出ボーナス係数の有無と値
- [ ] soul詳細UIにdreamバーを表示するか（スコープ外として別提案にするか）
- [ ] DreamQualityがビジュアル以外に新たな役割を持つか

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `0%`（提案書作成完了、実装未着手）
- 直近で完了したこと: 提案書ドラフト作成
- 現在のブランチ/前提: master

### 次のAIが最初にやること

1. 本提案書をレビューし、ユーザーの承認を得る
2. `docs/plans/` に実装計画を作成
3. `DamnedSoul`へのdreamフィールド追加から着手

### ブロッカー/注意点

- 蓄積・放出レートの具体値が未定（ユーザーと調整が必要）
- 既存のDreamビジュアルシステム（パーティクル、ポップアップ）への影響を最小化すること
- `rest_area_update_system`からDreamPool加算を削除する際、他の処理（fatigue/stress回復、退出、クールダウン）に影響しないよう注意

### 参照必須ファイル

- `docs/dream.md` — 現行Dreamシステム仕様
- `docs/dream-visual.md` — Dreamビジュアル仕様
- `src/entities/damned_soul/mod.rs` — DamnedSoul, DreamState, DreamPool定義
- `src/systems/soul_ai/update/dream_update.rs` — 現行dream蓄積ロジック
- `src/systems/soul_ai/update/rest_area_update.rs` — 休憩所Dream加算ロジック
- `src/systems/soul_ai/update/vitals_update.rs` — ストレス蓄積ロジック
- `src/constants/dream.rs` — Dream定数
- `src/constants/ai.rs` — REST_AREA_DREAM_RATE等

### 完了条件（Definition of Done）

- [ ] 提案内容がレビュー可能な粒度で記述されている
- [ ] リスク・影響範囲・検証計画が埋まっている
- [ ] 実装へ進む場合の `docs/plans/...` が明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-02-26` | `AI (Claude)` | 初版作成 |
