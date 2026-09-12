# ドア設計図の表示位置修正と正式反映への引継ぎ

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | `door-preview-alignment-plan-2026-09-11` |
| ステータス | `In Progress` |
| 作成日 | `2026-09-11` |
| 最終更新日 | `2026-09-12` |
| 作成者 | `Codex` |
| 関連計画 | [Door](production-door-art-plan-2026-09-05.md)、[型枠](provisional-wall-formwork-plan-2026-09-05.md) |
| 関連提案 / Issue / PR | `N/A` |

## 1. 目的

ユーザー提供画像の左上の支持壁・Door設計図の位置ずれを解消する。旧J1の機械判定と目視判定では
設計図の実画素を十分に確認できていなかったため、旧合格を修正後の表示承認へ流用しない。
修正後に進めるとのユーザー指示に従い、再検証後はWall generation 10 / Door generation 6の正式反映へ進む。

## 2. スコープ

- 対象: Door設計図・配置previewの表示経路、J1検証、関連仕様・Help影響レビュー。
- 非対象: 扉の形状、左右の開度、建築材料・時間、論理占有・保存形式の変更。

## 3. 現状とギャップ

- 旧job: `wall-door-joint-20260910T175148Z-bb4a4563`、subject `5de169bc`。
- Blueprintはgrid `(14,50)`、最初の支持壁は `(13,50)` / `(15,50)`。
- 旧observerはImage handle / Anchor / InheritedVisibilityと画面内座標だけを検査し、描画対象可視性・
  sprite alpha・実画素と支持壁の整合を検査していない。単なるROIの輝度分散は地形や隣接壁でも通る。
- 既存preview PNGは斜め方向の撮影画像。位置だけでなく実際にどの経路で描画されるかを先に確定する。

## 4. 実装方針

### 2026-09-12追加指摘: 設計図本体の斜め投影

ユーザーが「扉の設計図配置が斜め」と明示したため、材料表示とは独立した本体画像の修正を再開する。
現行rendererはcamera `(1.8,-2.5,2.2)` / ortho span 1.75で、ゲームのyaw 0・高度150 / offset90と不一致。
画像内anchor `(128,192)`も報告するだけで実投影を検証していなかった。材料位置の旧合格で本件を閉じない。

- Blender cameraをgame TopDownと同じyaw・傾斜へ揃え、縦補正もrenderへ反映する。
- 256px / 64wu canvasの1 tileを128px、地面中心を実測で`(128,192)`に固定する。
  NSの南端はcanvas下端へ達するため、旧斜視画像用bbox制限を無条件に緩和せず、
  明示した新projection契約と投影点の検査で全形状がcanvas内にあることを検証する。
- 旧release g6は不変。3 GLB・albedoと左右共通78°を維持し、preview 2 PNGだけを新世代へ更新する。
- ユーザーは修正のcommitと本番反映まで明示承認済み。旧アート承認は変更しない3Dにのみ引継ぎ、
  新画像を過去の目視承認済みと偽らず、今回の修正指示と新しい検証証拠を別記録する。
- Python回帰・Blender実投影点・PNG目視・通常authority J1で両軸とload後を確認し、旧画像の合格へ再ラベルしない。

以下は材料表示修正時の手順である。

1. 実機observerへ透明度・ViewVisibility・sprite geometryの検査を追加し、表示の成立条件を確定する。
2. 原因となる表示経路だけを修正。論理rootのgridや占有を表示都合で動かさず、診断専用変更は撤去する。
3. 完成Door / 支持壁 / previewの画素位置を照合する回帰を追加し、両軸・load後を再採取する。
4. 修正後J1が通ってから既存M4を進める。正式反映の条件付き指示を証跡と結び、旧画像を承認扱いしない。

Bevy 0.19のローカルregistryと既存実装をAPIの一次情報とする。描画APIは推測で変更しない。

## 5. マイルストーン

- [x] M1: 観測条件を追加。描画対象可視性・alphaを確認し、資材表示の支持壁への重なりを特定。
- [x] M2: 資材表示位置修正と回帰test。承認済み形状・開度は保持。
- [x] M3: 材料表示修正後のclean subjectで実機J1、独立verify、両軸・load後の画像を確認。
- [x] M4: 既存両計画へ結果・正式反映指示を引継ぐ。canonical登録と通常authority J1も完了。
- [ ] M5: 設計図本体の投影修正、回帰、新世代登録・通常版再受入。M1〜M4の材料表示修正とは別件。

M5実装: `door_preview_projection.py`とrendererがgameと一致する投影点を実測し、PNGだけを再生成。
EW/NS画像の目視・0.01px以内の投影照合・全vertexのcanvas検査はpass。Python workflow 121件もpass。
`seal_door_preview_revision.py`は不変3Dの承認と今回の画像修正許可を分離し、旧世代を上書きしない。
Help判断は`No impact`。`DoorAssetReadiness` → `ProductionDoorAssetPool` → `sync_door_preview_system` →
Blueprint root / pulse child / placement ghostの画像だけを更新し、操作・資材・配置条件・色と文言・保存の意味は不変。
前回の通常asset導入も表示だけであり、既存Helpに旧扉の色や半透明型枠へ依存する説明はない。
別作業の搬送回帰test・計画はこのcommitに含めず、cleanな検証treeで本件の全gateと実機受入を行う。

## 6. リスクと対策

- 隣接壁や地形をpreviewの証拠にしない。消去差分または対象固有の可視画素を検査する。
- 旧jobは履歴として保持。修正後の合格へ再ラベルしない。
- helperの初期選択や自動応答ではなく、今回のユーザーの「修正してください。その後、進めてください」を条件付き指示として記録する。

## 7. 検証計画

- focused regression、rust-analyzer、`python3 scripts/dev.py check`。
- `python3 scripts/dev.py cargo -- clippy --workspace --all-targets -- -D warnings`、`python3 scripts/dev.py verify`。
- native skillのdirect kitty、固定auditと6 checkpointを逐次実行し、独立verify・実画像確認。
- Help skillで実経路から分類、docs索引と`git diff --check`。

## 8. ロールバック方針

位置修正は表示責務に限定する。正式登録前にcanonical / runtime locatorを変更しない。
旧assetと証跡を保持し、既存M4の前像保存・pointer最後の切替手順へ従う。

## 9. AI引継ぎメモ

- 現在地: 資材表示の修正と両計画M4への引継ぎを完了。通常版への導入・残りの受入は両計画で追跡する。
  primary開始時HEAD `0889dd71`、再開時HEAD `7a0e4bf7`、いずれもdirtyなし。
- drawability検査subject `89e56bd1`のjob `wall-door-joint-20260911T003142Z-0872a031`を
  既存の専用worktree `wall-door-joint-83ad3f85`で完了。固定audit 3件 / screenshot 6枚と独立verifyがpass。
  manifest SHA-256: `1b25943cccccded737e4ddac462234986ae265b4bae572f48d2a592ddb4f5e6f`。
  ViewVisibility / alphaによる「描画対象外・透明」の仮説は棄却。ただし画素位置の合格とは区別する。
  旧成功jobと11 MiB capsuleは保持し、
  ユーザー指摘で表示受入を再開したため、worktreeだけをclean fast-forwardした。
- 資材表示の重なりは、従来offset `(20,10)`が東側支持壁へ入るため。
  Doorだけを南側支持tileの外へ配置する修正と回帰testを追加し、実機subjectへ反映した。
  rust-analyzerはerror / warning 0。native終了後、`cargo test -p hw_visual blueprint::material_display::tests`の2件がpass。
- 修正後の`python3 scripts/dev.py verify`がpass（Python 99 + 114件、workspace check、
  profiling / default workspace tests、追加profiling feature check、Clippy `-D warnings`、docs / diff gate）。
- Help判断: `No impact`。`hw_jobs::visual_sync::blueprint_visual_state`からmaterial displayへの
  実経路で、Doorの資材数・操作・成立条件・文言は不変。表示offsetだけを変えるためHelp本文は変更しない。
- 次の作業: 両計画M4の最新記録を参照。正式登録と通常J1 / Door品質は完了し、通常版導入・残件の完了後に本計画もarchiveする。
- 修正commit `42caef827681b42d63d341d03d8aea8878df3503`の再採取job
  `wall-door-joint-20260911T010631Z-738ce27a`は`2026-09-11T01:30:07Z`にvalid完了。
  `2026-09-12`に独立verifyと全6 PNGを確認。Intel Arc / Vulkan / X11 / High / DPI 1、
  固定audit 3/3、EW/NS両配置とload後に資材表示が支持壁から離れたことを確認した。
  新しいMemory / 性能matrixや通常releaseの受入を主張する証拠ではない。
  manifest SHA-256: `cc8cfccc78e043314e183af98fc018079890c6e2a32193c9a11e0217c0a4a2f4`。
  framed PNG: `c46fc1cd25d07a5edbe0e6d2fdec5bf3f7d66aefd3241d3cfbf4e71d5d0a70d7`。
  support-changed / support-restored / loaded PNGはbyte一致:
  `cfa64500873085fa1bb264d3cd826a23f00530e8ee1f66bb4e8002dfe5e75f8a`。
  capsule: `/home/satotakumi/Sync/hell-workers-assets/staging/acceptance/wall-door-joint-20260911T010631Z-738ce27a/`
  （14 file / 11 MiB、元jobと全file byte照合済み）。
- 指摘対象を材料表示と解釈した修正であり、壁・扉本体の位置を追加変更したとは判断しない。
  `2026-09-12`のユーザー指示は「再開してください」。新しい形状承認として扱わず、既存形状を保持する。
- 参照: `door_preview.rs`、`joint_actual_window.rs`、`hw_visual::blueprint`、`docs/help-screen.md`。
- DoD: 修正・全gate・新実機証跡・既存M4引継ぎ後にarchiveし、索引更新。
- worktree整理は両track close時。成功・失敗jobを今は削除しない。
- 通常authorityでもsubject `8d3edab9`、job `wall-door-joint-20260912T035627Z-c9ca85fe`の
  固定audit 3 / PNG 6 / 独立verifyがpass。全6画像で材料表示・支持壁・load後を確認した。
  manifest SHA-256 `b3af705ec6a2cf5bcfef4ec361f440181021bcec87c2adbc4e416e77c570cdfb`。
  capsuleは外部asset rootの`staging/acceptance/`同job名へ15 file / 12 MiBで保存し、全file byte照合済み。
  旧candidate capsuleを通常版画像へ再ラベルしていない。

## 10. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-09-12` | `Codex` | 材料表示修正後の実機6場面を再検証し、14 fileのcapsuleとhashを記録。通常release検証経路の整備へ進行 |
| `2026-09-11` | `Codex` | ユーザー指摘から位置ずれの調査・修正・再受入を計画化 |
