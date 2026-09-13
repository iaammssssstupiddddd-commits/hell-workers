# 検証データ管理の再レビュー・整理結果（2026-09-13）

対象はprimary `d960da6a`を基点とする規則・検証tooling変更と、既存検証領域。
[現行ワークフロー](validation-storage-workflow.md)を正本とする。

## 判断と修正

クローズした仕事の全検証データを保持する必要はない。初期設計の256 GiB / 7日 / 100 MiBと、
各batchのcapsule永続保存は撤廃した。従前のテスト成功は機構の動作を示しただけで、保持の必要性を
裏付けていなかった。218.6 GiBも整理前の蓄積であり、必要容量ではなかった。

8月RtT親計画はCompletedで旧比較の再測定をユーザー判断で停止済み。
直近wall surfaceも追加Capture / Memory / ちらつきを未検証のまま終了した。
現在の型枠release混在matrixは新しい状態の確認であり、旧job一式を入力にしない。
旧verifierの絶対path依存、過去文書のpath、親trackが未完という事情だけを保持理由にしない。

| 変更 | 現在の動作 |
| --- | --- |
| 結果確定 | sealは登録済みの独立verifierを元環境で実行し、結果を小さな台帳に記録。capsuleは作らない |
| 終了確認 | 用途のないjobは削除後にfinalize。終了済みarchiveの消失・変更を品質gateのエラーにしない |
| フィードバック | 同じcandidate / targetを最終承認・終了まで維持。無応答・review_at経過でも修正buildを止めない |
| 保持条件 | owner、具体的consumer、bytes、next_action、release_when。固定容量・日数の既定値なし |
| 新規出力 | 未登録jobと未整理batchを拒否。親のコード保全holdだけで子jobのraw全量保存を認めない |
| 保持範囲 | 最新PNGなど必要な子pathだけ保持可能。hold更新で後から生じた未知jobを既知化しない |
| 旧領域 | 用途を分類・不要分を削除し、reconcile後に未分類0件を確認 |
| 適用経路 | primary coordinator、native/perf入口、dev verify、7 agent入口、2 lifecycle、4 Skill adapterを同期 |

実行中processの保護、旧比較binaryの上書き防止、Capture→Memoryの逐次実行、
通常Cargoの永続storage / RAM guardは維持した。OS全体のquotaや無人削除は導入していない。
旧checkout自身への直接実行をOSで禁止する仕組みではないが、primary経由の起動と管理rootの残存確認を必須とする。

## 実施した整理

ユーザーの「ワークフローの修正と整理」の指示に基づき実施。
各rootのGit履歴・全差分・未追跡 / ignored、独自commit、assetの同一bytes、
processのcwd / open file / mmap、追加mountを確認した。

- primary内の旧独立clone 2個、関連worktree 3個（nested source-e4aを含む）、隣接旧worktree 10個を撤去。
  子から `git worktree remove` を使い、forceや広範囲git cleanは使っていない。
- 旧RtT RenderDoc / perf、legacy Cargo出力、完了したruntime / refactor / transport / wallのjob、
  採用済み候補の重複staging、終了済みwall surfaceの外部capsuleを削除。
- transportのprobeは完了計画で撤去済みの一時診断であり、別archiveを作らずrawとともに整理。
- primary通常Cargo cache / laneと、進行中型枠のcandidate / targetを維持。
  現行asset・release payload / receipt、復旧に使う旧generation / pointer前像は変更していない。
- 過去のpass / invalid / 打切り・未検証の判断は変更しない。削除済みpathは履歴参照となる。

### 容量

削除前後の `du -sx -B1` と `df -B1 /home` の実測。
Btrfsの圧縮・共有extentのため、du減少と空き増加は一致しない。

| 測定 | 結果 |
| --- | ---: |
| primary targetの減少 | 170.92 GiB |
| 隣接validation領域の減少 | 43.57 GiB |
| 外部capsuleを含む削除対象のallocated合計 | 214.51 GiB |
| filesystemの空き | 548.88 → 695.75 GiB（約146.87 GiB増加） |
| filesystem使用率 | 43% → 27% |
| 整理後の管理領域（保全分を含む） | 4,881,354,752 bytes（約4.55 GiB） |
| legacy未分類 | 0件 |

残る登録worktreeはprimaryと `hell-workers-validation/wall-door-joint-83ad3f85` の2個。
終了した旧jobの再検証のためのRust targetは残していない。

### 残すものと用途

| path | allocated bytes | 用途・終了条件 |
| --- | ---: | --- |
| 隣接 `wall-door-joint-83ad3f85` | 4,879,749,120 | 未実施の型枠release混在quality/DPI matrixとフィードバック。終了後、他consumerがなければ撤去 |
| `target/native-acceptance/wall-art-20260901T055138Z-f93b0b12` | 614,400 | 登録済みwall reference validatorが読むmanifest / observation / PNGの3 fileのみ。reference契約の廃止・移行時に整理 |
| `.git/validation-storage/preserved-work-20260913` | 991,232 | 独自コードと少量の制作原本。所有・採用先確認後に正本へ引継ぐ |

旧P00 / RenderDoc cloneにprimary未到達のcommitが37 / 48件あったため、
全ref・各detached HEADをprimaryの `refs/archive/validation-disposal-20260913/` 配下へ引継ぎ、
元repoの全commitがprimaryで到達可能なことを確認した。隣接previewの独自commit `d81e0a2f` も参照を保全。
これはコード履歴の保全であり、検証rawやbuild成果物の保存ではない。

RenderDocの未コミット5ファイルは上記保全先へコピーしてSHA-256照合し、
基点 `10763a4d` のpatchも保存・reverse apply check済み。
旧壁案の固有blend 4点と出自metadataを保全した。採用v4原図・最終blend、
Door原本、旧checkoutのassetは既存正本との一致を確認して重複を整理した。

削除一覧はlocal `.git/validation-storage/disposal-20260913.json`（458 entries）に記録した。
SHA-256は `4905c89866228f26ed395cc6ce82356766144c4dad45ee16c52ae7fef0c1ee92`。
保全manifestのSHA-256は `cc312c1c28617bda291cb78679d7ef49661aeb67b24fa267f936d55af8231e3d`、
壁原本inventoryは `1947aa2848fd1d1a69138822353e6d7dbb455c2e4d9ec52e29b34673bb645f3f`。
一覧は整理経過のmetadataであり、削除したjobを復元・再検証できるとは主張しない。

## 検証とHelp影響

整理段階の `python3 scripts/dev.py verify` はpass。Python tooling 134件（storage 29件）、
Blender tooling 151件、perf self-test、storage / 規則 / Help / docs、workspace check、
profiling各feature、Clippy全target `-D warnings`、workspace tests、diff hygieneを含む。
独立レビューの指摘だった予算未指定での起動、親holdによる未知job隠蔽、PNGだけ残す終了、
hold更新によるsnapshot拡張、親コードholdによるraw全量保持は修正・回帰test済み。
native Skill指定の8 self-test、adapter同期、quick_validateもpass。

旧staging撤去後にWall generation 13を `repo=None` で単独validateしてpass。
保存したpromotion plan / snapshotによるg10へのrollback計画もread-onlyでpassし、実際には戻していない。
管理checkは予算なし・legacy未分類0・約4.55 GiBでpassした。
整理段階では新規実機job・validation worktreeを作成せず、一時計画を撤去して索引を更新した。

Helpは `crates/bevy_app/src/interface/ui/help_content/mod.rs::build_help_panel_content` が
静的 `manifest::feature_specs()` / providerから構築し、検証台帳・jobやdocsをruntimeで読まない。
今回の変更は検証Python入口と整理規則のみで、ゲームの入力・建設搬送条件・表示・runtime assetは不変。
実経路の判断は **No impact**。Help source / snapshotの空変更は加えていない。

## クローズ（2026-09-13）

ユーザーのクローズ指示により、保存規則の修正・既存領域の整理・フィードバック用ビルドの改善を完了する。
修正確認は `dev.py feedback` と合同native helperの `--feedback` を使用し、既存dev cacheを再利用する。
正式受入・性能計測の条件は維持し、軽量結果を正式passへ転用しない。
ルール・2 lifecycle workflow・Cursor正本からCodex / Gemini / ClaudeへのSkill同期は確認済み。

後続のビルド改善では、既存cacheで未変更0.40秒、main.rsの1行変更3.29秒を実測し、診断変更を撤去した。
Intel Arc / Vulkan / X11の合同feedbackは6画面・状態遷移がpassし、正式verifyがfeedbackを拒否することも確認した。
初回の描画前captureによるinvalidと修正後passの2 batchは、いずれもseal / finalize済み。
不要job計12,062,720 bytesを削除した。詳細な条件・制約は[開発ガイド](../DEVELOPMENT.md)を参照する。

改善後の最終 `check` / `verify` はpass。Python tooling 137件、Blender tooling 151件、
全Rust test、Clippy警告0、native関連self-test 9件を確認した。
今回の専用検証環境・一時計画・未整理batchは残っていない。クローズ時のstorage checkもpass。
前掲の3保持対象はそれぞれ別の継続用途・担当者・終了条件へ引継ぎ済みであり、
このワークフロー修正のクローズを型枠の未実施matrixや原本の破棄判断に読み替えない。
通常開発用のCargo cacheは引き続き使用する。
