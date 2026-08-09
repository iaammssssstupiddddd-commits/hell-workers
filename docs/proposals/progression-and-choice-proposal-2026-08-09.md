# Track D 進行と選択提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `progression-and-choice-proposal-2026-08-09` |
| ステータス | `Draft` |
| 作成日 | `2026-08-09` |
| 最終更新日 | `2026-08-09` |
| 作成者 | `Codex` |
| 関連計画 | `TBD`（採用・初回スコープ決定後に作成） |
| 移管元 | `docs/proposals/archive/gameplay-management-improvements-proposal-2026-07-17.md`（旧Track D） |
| 関連Issue/PR | `N/A` |

## 1. 背景と問題

操作、フィードバック、運営ポリシー、復旧・永続化の基盤は、旧総合提案のTrack A〜Cで段階的に整備された。
一方、Dreamは資源として存在するものの、継続的な判断や中長期目標へ結び付く用途が少なく、Familiarの階級も
世界観上の記述が中心である。

旧総合提案ではこれらをTrack Dとして扱っていたが、Track A〜Cの完了記録と、未採否の進行システム設計を
同じ提案で管理すると、親提案を閉じられず、Track Dの採否・初回スコープ・検証判断も曖昧になる。
本提案はTrack Dだけを独立させ、Dream Edict、Contract、Familiar昇格を一つの進行設計として評価する。

## 2. 目的（Goals）

- Dreamの支出を、短期的利益と明示的な代償を持つ運営判断へ変える。
- Contractにより、日々の建築・物流・管理実績を中長期の目標へ接続する。
- Familiar昇格により、Soulを直接操作せずマクロ指揮能力を段階的に拡張する。
- 進行状態、期限、効果範囲、報酬を安全に保存・再構築できる境界を定義する。
- D1〜D3を独立して採否・実装できる依存関係と、最小vertical sliceを示す。

## 3. 非目的（Non-Goals）

- D1〜D3を一括実装すること。
- Soulを一体ずつ直接操作するゲームへ変更すること。
- 本提案段階でバランス数値、最終アート、Contract全系列を確定すること。
- Track A〜C、HVAC/Plumbing、3D RtTなど既存提案を再設計すること。
- 進行表示のためにAI候補評価、経路探索、全Entity走査を追加すること。
- 失敗を即ゲームオーバーとして扱うこと。

## 4. 提案内容（概要）

一言要約: **Dreamを代償付き方針へ、運営実績をContractへ、Contractと管理実績をFamiliar昇格へ接続する。**

| 区分 | 内容 | 主な成果 |
| --- | --- | --- |
| D1 | Dream Edict | Dreamを消費する期間・範囲限定の運営方針 |
| D2 | Contractとマイルストーン | 期限、代償、複数解法を持つ中長期目標 |
| D3 | Familiar昇格 | 既存のマクロ指揮を拡張するdurable rankと特性 |

D1はD2から独立して導入できる。D3がContractを昇格条件に使う場合だけ、D2をD3の前提にする。
新しいworld永続データは、Track C3のpreflight・phase-aware rehydrateとTrack C2の保存契約を利用する。

## 5. 詳細設計

### 5.1 仕様

#### D1. Dream Edict

- Dreamを消費して、期間限定または範囲限定の運営方針を発令する。
- 初期候補は、効果と代償が明確な少数に限定する。
  - `Overtime`: 一時的に作業効率を上げるが、疲労増加を強める。
  - `Mandatory Repose`: 休息を優先し、短期生産を下げて疲労を回復する。
- 発令時に影響範囲と対象集合を固定するか、再評価規則を明示し、別world・別ownerへ効果を漏らさない。
- 実行中タスクを強制破棄せず、既存の安全なタスク終了・再判断境界で効果を反映する。
- active Edictは種別、期限または残り時間、効果範囲を永続化する。範囲はtransient Entity IDではなく、
  durable ownerまたはfootprintで表し、ロード時に再検証する。
- セーブ/ロードや一時停止で効果時間や代償をリセットできないよう、期限のclock ownerを先に確定する。

#### D2. Contractとマイルストーン

- GameTime、既存のtyped outcome/event、人口・物流統計から評価可能な期限付きまたは継続目標を導入する。
- 同じ出来事を複数observerが二重計上しないよう、Contract progressの唯一のconsumerとevent identityを固定する。
- 失敗をゲームオーバーにせず、報酬減少、次候補の変化、世界観上の演出で扱う。
- 報酬はDream、称号・印章、建築・Edict・Familiar昇格の解禁を中心とする。
- 初期チュートリアルをContractの特殊系列として表現できる余地を残すが、初版vertical sliceの必須要件にしない。
- 単純なチェックリストではなく、期限、代償、複数解法、失敗後の変化を持つ少数の手作りContractから評価する。

#### D3. Familiar昇格

- 世界観上の`Imp`、`Servitor`、`Greater`、`Overseer`を段階的なFamiliar rank候補として扱う。
- 昇格条件はContract、管理実績、Dream・印章など、観測可能で再計算可能な進行値へ結び付ける。
- 効果は管理Soul上限、活動範囲、作業専門化など、既存のマクロ指揮を強化する方向に限定する。
- ランクと選択した特性は永続化し、Track B2の`FamiliarOperation` / `FamiliarPolicy`と責務を分ける。
- 昇格後も既存の担当・予約・実行中タスクを直接破棄せず、方針再評価の安全な境界を利用する。

#### 依存関係

```text
Track C3 / C2 の保存・再構築境界 ─── D1 / D2 / D3 のdurable state

D1 Dream Edict ───────────────────── 独立導入可能
D2 Contract ─────────────── D3 Familiar昇格
                             （Contractを昇格条件に使う場合のみ）
```

Track A1の入力所有権、A2の通知経路、A3の説明可能な状態表示は再利用する。A2に残る独立した重点実機受入は
本提案の設計判断と分離して管理し、Track D実装時には利用する通知経路の現行契約を再確認する。

#### 受入条件

- Dream支出が短期的利益と明示的な代償を持ち、対象外world・ownerへ漏れない。
- Contract進捗が同じ出来事を二重計上せず、失敗を含めてセーブ/ロード後も保持される。
- Familiar rankは管理能力を拡張するが、Soulの直接操作を必須にしない。
- Edict期限、Contract進捗、rankの保存・ロード・rollback・recovery-only loadで意味が変わらない。
- UIを開閉しても進行判定回数、AI候補評価、経路探索回数が増えない。

### 5.2 変更対象（想定）

- 共有型・永続状態: `crates/hw_core/src/`
- Dream・時間・進行判定のowner: 現行実装を調査して責務crateを確定する。
- Familiar rank適用: `crates/hw_familiar_ai/src/`、必要なroot adapterだけ`crates/bevy_app/src/`
- UI/Intent: `crates/hw_ui/src/`、`crates/bevy_app/src/interface/ui/`
- 保存・再構築: `crates/bevy_app/src/systems/save/`とTrack C3 registry登録元
- 恒久ドキュメント: `docs/dream.md`、`docs/familiar_ai.md`、`docs/world_lore.md`、
  `docs/save_load.md`、`docs/state.md`、`docs/events.md`、`docs/invariants.md`、`docs/help-screen.md`

### 5.3 データ/コンポーネント/API変更

追加候補:

- `DreamEdict`
- `Contract`
- `ContractProgress`
- `FamiliarRank`
- Edict / Contract / promotionのtyped request・outcome

実装計画作成前に、現行Dream、GameTime、Familiar設定、通知、保存registryとの型重複を再調査する。
進行値をUI専用summaryやtransient Entity IDだけで表さず、durableな正本とruntime派生値を分離する。

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| Contract・昇格だけを先に追加する | 不採用 | 運営手段と結果通知を利用できる現行基盤へ接続せず、単純な目標一覧になりやすい |
| D1〜D3を一括実装する | 不採用 | 遊びの判断密度を評価する前に永続schemaとUIの変更範囲が大きくなる |
| D1を独立vertical sliceにする | 検討 | 既存Dream消費と運営判断の接続を最小範囲で評価できる |
| D2を独立vertical sliceにする | 検討 | Contractのデータ所有とevent計上を先に評価できるが、初期コンテンツ設計が必要 |
| 進行状態をUI表示時に再計算する | 不採用 | UI開閉がsimulation負荷と判定結果へ影響する |

## 7. 影響範囲

- ゲーム挙動: Dream消費、期間・範囲限定効果、Contract進捗、報酬、Familiar能力が増える。
- パフォーマンス: event-drivenな進行集計と有界なactive stateを基本とし、UI用の再探索を禁止する。
- UI/UX: Edict選択、Contract進捗・結果、昇格条件・結果を表示する画面が必要になる。
- セーブ互換: D1〜D3のdurable state追加は、Track Cのv1 additive方針で収まるかを実装計画前に判定する。
  rename、field shape変換、削除が必要ならcontainer v2 / world schema migrationを別計画化する。
- AI/物流: Edictやrankが既存score・capacityへ影響する場合も、Track Bの正本と安全な再判断境界を再利用する。
- Help: 新しい操作、条件、結果は実装と同時にHelp manifest/provider/coverageへ追加する。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| Contractが単純なチェックリストになる | 世界観と運営判断が弱い | 期限、代償、複数解法、失敗後の変化を持つ少数の手作りContractから始める |
| Edict対象がload後に別ownerへ漏れる | プレイヤー意図と効果が変わる | durable owner・footprintとload時preflightを使い、transient Entity IDを正本にしない |
| セーブ/ロードで期限を延長できる | Dream消費の意味が失われる | clock ownerと保存値を固定し、pause/load境界のfixtureを先に作る |
| 進行eventを二重計上する | Contractを意図せず達成する | event identityと唯一のprogress consumerを固定する |
| 昇格が既存方針・担当を破壊する | 予約不整合や幽霊担当が生じる | Track B2の方針正本と既存task terminal/re-evaluation境界を維持する |
| P2が不要な前提を待ち続ける | 進行要素が届かない | D1〜D3ごとに採否し、保存・入力・通知以外の不要な依存を作らない |

## 9. 検証計画

- pure rule: Edict適用・期限境界、Contract event重複排除、昇格条件と効果を単体テストする。
- save/load fixture: 旧save補完、新state round-trip、rollback、recovery-only load、期限・進捗・rank不変を確認する。
- fixed tick: 効果開始・終了、Contract成功・失敗、rank適用のexactly-onceを確認する。
- UI/input: foreground capture、stale intent拒否、typed outcomeとHelp導線を確認する。
- performance: UI開閉前後で進行判定、AI候補評価、経路探索の回数が増えないことを確認する。
- player acceptance: Dreamの利益と代償、Contractの複数解法、昇格によるマクロ指揮の変化を実機で確認する。
- 実装完了前に`python3 scripts/dev.py verify`と、必要なactual-window/native acceptanceを通す。

## 10. ロールアウト/ロールバック

- 採用後はD1、D2、D3を独立計画へ分割し、選んだvertical sliceだけを先に実装する。
- 新しい進行データは旧saveに明示的な既定値を補う。
- Edict / Contract / rankの機能境界を分離し、未採用サブトラックを他サブトラックから参照しない。
- 新形式を旧実装が安全に無視できない場合はformat/schema mismatchとしてload前に拒否し、live worldを保持する。
- 問題が出た機能を無効化しても、既に消費したDreamや付与済み報酬を暗黙に巻き戻さない。

## 11. 未解決事項（Open Questions）

- [ ] Track Dを採用するか決定する。
- [ ] 初回vertical sliceをD1とD2のどちらから始めるか決定する。
- [ ] Dream Edictの初回効果対象をWorld全体、Room、Familiar管轄のどれにするか決定する。
- [ ] Contractを手書きRust定義、RON asset、別のdata sourceのどれで管理するか決定する。
- [ ] Edict期限のclock ownerと、offline timeを進めるかを決定する。
- [ ] Familiar rank候補と初版効果を世界観・バランス観点から確定する。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `Draft / 0%（未採否、実装計画未作成）`
- 直近で完了したこと: 旧総合提案からTrack Dの目的、D1〜D3、依存、リスク、検証、未決事項を移管した。
- 現在のブランチ/前提: Track A〜Cの基盤と完了記録を再利用する。A2の重点実機受入は別計画の残件である。

### 次のAIが最初にやること

1. `docs/dream.md`、`docs/familiar_ai.md`、`docs/world_lore.md`と現行Dream/GameTime/save型を再監査する。
2. D1 / D2のどちらを初回vertical sliceにするか比較し、採否・スコープを本提案へ記録する。
3. 採用時だけ`docs/plans/`に実装計画を作り、D1〜D3を一括実装しない。

### ブロッカー/注意点

- Track Dは未採否であり、型名・rank名・Edict例は設計候補である。
- 旧総合提案を再開せず、本提案をTrack Dの唯一の現行正本とする。
- C1/C2/C3のsave・rehydrate契約を弱めない。
- UI表示のためにAI候補評価、経路探索、全Entity走査を追加しない。
- production変更後は`hell-workers-review-help-impact` Skillの判断を必ず行う。

### 参照必須ファイル

- `docs/dream.md`
- `docs/familiar_ai.md`
- `docs/world_lore.md`
- `docs/save_load.md`
- `docs/state.md`
- `docs/events.md`
- `docs/invariants.md`
- `docs/proposals/archive/gameplay-management-improvements-proposal-2026-07-17.md`
- `docs/plans/archive/save-rehydration-registry-plan-2026-08-03.md`
- `docs/plans/archive/save-catalog-autosave-plan-2026-08-03.md`

### 完了条件（Definition of Done）

- [x] Track Dの目的、非目的、D1〜D3、依存関係が独立提案として記述されている。
- [x] リスク、影響範囲、検証計画、ロールバック境界が記述されている。
- [ ] Track Dの採否と初回vertical sliceが決定されている。
- [ ] 採用するサブトラックの`docs/plans/...`が作成されている。

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-08-09` | `Codex` | 旧総合提案のTrack Dを独立移管。Dream Edict、Contract、Familiar昇格の依存、未決事項、検証境界をDraftとして整理 |
