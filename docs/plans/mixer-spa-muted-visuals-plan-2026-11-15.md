# ミキサーの彩度とSoulSpaの判読性を改善する

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | mixer-spa-muted-visuals-2026-11-15 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-15 / 2026-11-15 |
| 作成者 | Codex |
| 関連Issue | 利用者の追加フィードバック / TAK-14 |

## 1. 目的

ミキサーの鮮やかな橙色を抑え、SoulSpaを骨の枠と四つの区画として読みやすくする。
通常画面で世界観と調和することを採用条件とし、過去の採用を今回の外観承認へ流用しない。

## 2. スコープ

Mixer v6とSoulSpa v2の明示weathered-v1を追加する。旧styleの再現を維持する。
最初の仮説は、形状を保ったままドラム色と骨/区画の描き分けで改善できること。
通常/遠景で不足なら同系統の色調整を反復せず、形状の別proposalへ進む。
RestAreaを含む他7種、Bridge、精製clock、四区画の論理・実worker発光、save/gameplayを変えない。

## 3. 現状

Mixer g5のdrum素材は彩度が高い。SoulSpa g3は低い骨枠と平坦な円が同じ面に見える。
既存mode4は実際に接続済み。shader全体tintではなく部品localの色面と主要輪郭を改善する。

## 4. 実装方針

同目的branch codex/tak14-non-bridge-release、基点25989dbbb0105188868c81d6d765dc67efcc275a。
mainだけが編集/build/test/Git/原本を所有する。独立reviewは既存のread-only担当へ依頼する。
旧手描き素材を保ち、新styleの外側drumだけ低彩度化する。口・溶接線・架台は維持する。
Spaは骨の節と外周を暗線/骨色で分け、区画は暗い面と控えめな紋様を持たせる。
実emission atlas、mesh/UV/parts/寸法/alphaを維持する。新候補に旧承認をコピーしない。
公開/push/mergeは含めない。

## 5. マイルストーン

- [ ] 明示styleの生成/検証/拒否条件と旧出力再現を実装する。
- [ ] 新原本とArtPreviewを生成し、通常/遠景・Mixer回転・Spa状態を独立審査する。
- [ ] 新しいart/numeric/release、復元確認、正式install/既定接続を行う。
- [ ] 導入後の実画面・通常startup、Help、変更別CI、storage、仕様/原本同期を完了する。

## 6. 検証

実atlasの彩度/部位限定差分、旧style hash、Spa emission不変、誤kind/style/stageの起動前拒否。
原本生成・nativeはprimary dev.py validationへ登録し、既定direct kittyで実行する。
現行source/headで独立verify/seal/finalizeを行い、画像は全て目視する。
今回がpixel-onlyでmesh/UV/画像寸法/alpha/材質/state/常駐資源量が全て同じなら、実windowでそのparityを照合する。
形状・shader・容量に差があれば必要な性能gateを再実行する。旧性能passを新sourceの計測済みとは報告しない。
Help実レビュー後、dev.py ci check --base <基点> --mode auto、git diff --checkを行う。

## 7. データ管理と終了

正本はdocs/development-infra/validation-storage-workflow.md。
owner Codex、consumer Mixer/Spa visual feedback 2026-11-15。primaryと既存Cargo cacheを再利用する。
新staging/jobはexact path・owner・consumer・次用途・release_whenを登録する。
旧承認済み原本900件とcanonical/runtime/codecを保全し、新unique原本を正本へ集約してから重複jobを整理する。
採用結果を恒久仕様へ反映し、計画を撤去、両索引とstorage checkを確認する。

## 8. 引継ぎ

現状は調査済み、実装開始。未配車・未生成・旧binding維持。
旧統括/Run/helperは起動しない。承認・技術pass・利用者の採用判断を混同しない。
