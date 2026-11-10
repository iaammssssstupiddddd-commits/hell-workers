# Tank候補の通常保存・復元受入

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `tank-candidate-persistence-plan-2026-11-10` |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-10 |
| 作成者 | Codex |
| 関連提案 | N/A |
| 関連Issue/PR | TAK-14、橋以外9種の正式導入の継続 |

## 1. 目的

Tank候補の通常施工・満水・給水精製は限定受入済み。次に通常のManual1 Save/Loadで、
実Water50個、専用bucket5個、BucketStorage2個と所有関係・候補表示が復元されることを検証する。
給水結果は[物流仕様](../logistics.md#44-mudmixer-水搬入-deliverwatertomixer)を正本とする。

## 2. スコープ

対象は明示profilingの専用`m2-tank-candidate-persistence` case、実commitの読取receipt、
独立verifier、拒否系回帰、fresh actual-window。準備はcamera/pauseだけで、材料・actor・task・Waterは投入しない。
通常save schema・入力・gameplay・asset正本・承認権限は変更しない。
移動・撤去・Memory/性能・正式promotion/install/release・他8種はこの限定passで閉じない。

## 3. 現状とギャップ

既存Full recipeには保存入力とload後の同一性検査がない。実commit receiptのmapはSoul/RestAreaとSpaを
別管理しており、Tank/Water/bucket/companionを証明しない。既存mapを拡張解釈せず専用mapを追加する。

## 4. 実装方針

- mainが編集・検証・Gitを所有し、既存read-only reviewerを再利用。
- 同目的branch `codex/tak14-non-bridge-release`、基点`db136cf08a034260d158ad1e86e6ff003acc2f2e`。
- 専用case/scopeだけにreceiptを追加し、実loadのEntityHashMapを完全・一対一・有界で記録する。
- 保存前Full prefixとload後prefixを分離し、全入力・全rawの間を省略しない。実bytes/hash・request・epochを照合。
- save/loadによるruntime task resetを継続作業に読み替えず、元owner消失・新ownerの所有/逆集合・同asset identityを確認。
- Bevy 0.19の既存commit/collector・catalog入力・loaderを使う。外部公開/push/mergeはしない。

## 5. マイルストーン

### M1: 実保存receipt

- [x] 専用scopeのtyped M2 snapshotと完全remap、欠損/重複/上限/旧scope不混入のRust回帰（保存観測12件pass）。

### M2: 独立driver/verifier

- [x] 通常施工/Fullから実F5/F9、選択/確認入力へ接続（実機passとは別）。
- [x] bytes・entity/owner/inventory・候補表示・全prefix・拒否系回帰。legacy入力edgeは実messageとの一対一対応、実施工storage IDを固定。
- [ ] Help No impactを実経路で確認し、変更別CI/check/Clippy/RAとexact独立レビュー。

### M3: 実機と整理

- [ ] primary validationのfresh plan/kitty/verify/seal/finalizeと全PNG目視・独立raw review。
- [ ] 成功・失敗・未検証を既存正本へ集約、用途終了jobを整理してstorage check。
- [ ] 本一時計画を撤去/索引再生成。9種全体の未達条件を維持。

## 6. リスクと対策

| リスク | 対策 |
| --- | --- |
| genericmapからTankの復元を推測 | 実mapのselected型を独立inventoryと完全照合 |
| pause中のruntime resetを旧状態へ修復 | 実durable/reset契約だけを検査、観測専用で非修復 |
| 旧prefixのcutoffが不正を隠す | 新caseに明示し、後続は全sample/全入力を別検査 |
| 検証成功をreleaseへ昇格 | authority=false、候補と正式releaseを分離 |

## 7. 検証計画・データ管理

`dev.py cargo`、`dev.py check`、基点からの`dev.py ci check --mode auto`、通常/profiling Clippy警告0。
native/Help Skillとprimary storage正本を使用。native登録からsealまでsource/harness/assetsを凍結。
既存Tank候補`target/native-acceptance/tak14-tank-isolated-candidate-20261109-v1`（204800B）と
codec `target/native-acceptance/tak14-m3-codec-20261107-v1`を保持し、同じCargo targetを使う。
新jobは`target/native-acceptance/tak14-tank-candidate-persistence-20261110-v1`。
owner=TAK-14、consumer=Tank ordinary candidate persistence acceptance。
登録済み独立verifier・画像/raw review・結果同期後にjob consumerを解除し撤去する。
候補原本/codec/cacheは9種導入の別consumerがあるため保持。旧v22は限定passを正本へ記録してjob整理済み。

## 8. ロールバック

本補正単位。未知の並行差分・旧invalid・採用原本は上書き/撤去しない。

## 9. AI引継ぎ / Definition of Done

利用者は橋以外の継続を指示済み。追加の進行確認は不要。旧統括は再開しない。
現在M1とM2実装/fixtureを完了し、exact reviewと実機の前段gate中。通常/profiling workspace test・Clippy、
tooling793件・Blender229件を含む変更別gateは一度passし、最終型guard追加後はfresh gateを行う。
Help No impact: 明示profilingの非保存診断と専用native入力/verifierだけを追加し、通常の操作・文言・
save schema・gameplay・asset authorityは変更しない。M1〜M3の全完了、同subjectのHelp/native/独立review/storageを要求する。
fixtureや部分passを全体完了へ読み替えない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| 2026-11-10 | Codex | 通常満水Tankの保存・復元に限定して着手 |
