# TAK-14 統合PRの対象と検証境界

## PR本文

タイトル: TAK-14製品成果・受入基盤と通常開発ルールの統合

### 概要

- 建設物のpresentation、世代・fallback・poolとBridgeのpreview/commit契約を統合する。
- production受入の登録adapterを明示引数にし、Capture/Memoryの取り違え、古いidentity、未認証receipt、合成された観測を拒否する。
- profiling観測を排他的な一時出力へ統一し、失敗成果・symlinkを上書きしない。
- Bevy 0.19 / Parley 0.9のAPIを維持した日本語辞書backportと表示・編集回帰を追加する。
- 開発用Portal入力でOSが返すsingle-use許可を安全に継続利用する。失効・context変更・不明結果では再送しない。
- vendorと検証helperをsource fingerprint、Help分類、storage guardへ含める。
- 廃止済み独自Orca統括を再導入せず、通常Orcaとmain所有の検証・Git運用を正本にする。

### 比較範囲とレビュー

branchは`codex/tak14-product-integration`、比較基点は
`b68bafd7358580ff8ad77962fa02987a215b5b60`。
着手時のcommitted差分は365ファイルであり、TAK-14だけの小PRではない。
既存R01〜R14、静止アート参照、CI/storage、通常Orca文書の履歴も含む集約PRである。
独立read-only監査は変更production hunkと主要呼出経路を累積で確認した。
未変更の全leafや全testの逐行監査、正式release承認を意味しない。
無関係なステージ済み`package.json` / `package-lock.json`は対象外とし、その内容とstageを維持する。

### 検証

最終コミットに対して以下を実行する。実際の結果とtested HEADは最終報告で照合する。

```bash
python3 scripts/dev.py ci check --base b68bafd7358580ff8ad77962fa02987a215b5b60 --mode auto
python3 scripts/dev.py docs --check
python3 scripts/dev.py validation check
git diff --check
```

変更別CIはcontracts/tooling/deps/rustの全4群、default/profiling workspace test、
Memory/Tracy/RenderDoc compile、Clippy警告0を対象とする。
rust-analyzer MCPは`diagnostics:null / Unexpected response format`で取得不能だった。
IDE診断0件とは記載せず、コンパイラ・Clippyの結果を代替根拠として明示する。

実機で取得したcatalog描画とrefactor-suiteのPortal入力feedbackは限定証拠である。
前者は日本語表示と旧ICU4X日本語辞書欠落errorの解消、後者はrename/search/fold、
progress caller、Save/Loadと取消を確認した。後続sourceの正式受入へ流用しない。
OS許可は初回grantの後、別jobで追加操作なしに復元できた。永久許可とは主張しない。
最新画像と比較入力はprimary validation台帳の具体的consumerで保持する。

### Help判断

既存Bridge/R01〜R14のplayer-facing変更はproviderとexact snapshotの更新を維持する。
今回の追加はNo impact。日本語backportは既存表示・編集の欠落を修復し、明示profiling観測、
登録adapter、OS入力transport、storage拒否guardは開発検証だけに作用する。
新しいInputAction/UiIntent、通常の文言、gameplay、save schema、Bridge/Site条件は変更しない。
Bridge検証候補は本番の`building_spawn_pos`と既存`Site.contains`を満たすものだけへ限定する。

### 既知の制約・後続

- 利用者判断により新しいBridge/Site配置仕様は延期する。通常生成seed20260914では合法な川crossingと既存Siteが交差せず、検証は拒否する。この条件を回帰testで固定し、通常配置・建設・通過・保存復元・解体の実機成功とは扱わない。
- 認証されたordinary-world収集producerとhost instrumentsが未提供の正式production campaignはfail-closedのまま。旧Runや技術preview passを代用品にしない。
- 今回は性能改善量や全10種のMemory/GPU/native/RSS budget達成を主張しない。profiling構成のcompileは計測ではなく、定量campaignはreleased identityと実producerが揃った全体受入で実施する。
- 独立numeric/art採否、promotion/install/release/rollbackと全consumerの正式世代受入は未完。[原移行計画](../plans/3d-rtt/non-wall-floor-building-art-migration-plan-2026-09-19.md)を維持する。
- PR作成、追加push、merge、Linear Doneはこの準備工程では実行しない。TAK-14全体完了を宣言しない。

## 保存・整理

取り込み元TAK-14資料は[制作workflow](../assets_workflow.md)に記録した正本・保全先を維持する。
旧18作業場の整理と復旧stashはarchiveの統合計画に記録済みで、旧workflowを再開しない。
PR準備の一時計画は履歴へ移し、本書と各機能の恒久仕様を参照先とする。
