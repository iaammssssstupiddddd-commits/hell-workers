# 建設設備の世界観への統一

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | building-world-style-2026-11-14 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-14 |
| 作成者 | Codex |
| 関連Issue | TAK-14 導入後の画風フィードバック |

## 1. 目的

明るい木肌・新品の布・現代的な金属色を、世界観の黒ずんだ木、錆鉄、煤けた布と整理したインク線へ揃える。用途の読みやすさと直前に採用した形を維持する。

## 2. スコープ

Tank、MudMixer、RestArea、OutdoorLampの新しい塗りとworld/catalog画像を制作・独立採用・正式導入する。SoulSpaの骨と紫の既存表現を参照し、黄色い網の除去と入所Soulの非表示は維持する。形状、数値、状態、容量、照明挙動、anchor、画像寸法、UV、pool、Bridge、rendererは変更しない。紫を全設備共通の発光装飾にはしない。

## 3. 方針と比較基点

正本はworld_lore.md §6とbuilding-art-direction.md。共通のinfernal-v1塗りを新規候補へ明示適用し、legacy paintは再現用に残す。新manifest generationと新しい原本を用い、旧g2とrollback情報を保全する。同目的branch codex/tak14-non-bridge-releaseを再利用し、基点は0c4b9b05a98e4853b0b7604f5c87b79218ae3871。mainが編集・検証・Gitを所有し、独立reviewはread-only。公開操作は行わない。

## 4. マイルストーン

- [x] 共通palette、素材別描線、legacy互換、原本依存hashを実装する。
- [x] normal/farのactual-windowで明暗差と施設の判読を確認する。
- [x] geometry/UV/部品/状態/画像寸法の不変を実データで確認し、新4件の独立art/numeric/release採用を得る。
- [ ] guarded promotion/install/rollback確認後、既定bindingと恒久仕様を更新する。
- [ ] GNU startupの大きな一時確保の保持を補正し、新sourceのgalleryと固定予算の全24性能試行を通す。
- [ ] Help実レビュー、変更別CI、Clippy、独立review、primary storageと一時データ整理を完了する。

## 5. 検証

塗りの再現性、旧paint互換、geometryとalphaの不変をfocused testsで確認する。nativeはprimary dev.py validationとnative acceptance Skillを使い、閉じた過去jobを再開しない。新source/jobで受け入れる。albedoだけの変更であることを実inventoryで照合し、以前の性能passを新sourceの測定結果へ付け替えない。完了gateはci check --base 0c4b9b05a98e4853b0b7604f5c87b79218ae3871 --mode auto。分類不明はfull/verify。

## 6. リスク・終了・引継ぎ

暗く塗り潰さず、土上のsilhouetteと水位・開口・点灯の区別を優先する。旧数値契約は変えず、原本とcanonicalの旧世代を保全する。採用結果を恒久仕様へ集約後、この計画を削除し索引を再生成する。検証開始・再開時はvalidation-storage-workflow.mdを読み、保持consumerと終了条件を登録し、未使用のjobを整理する。旧Orca統括/Runは再開しない。

## 7. 実測後のRSS受入補正

4件のg3正本・既定binding、16操作のg2復帰、原本33組303ファイル、既定9種galleryは独立採用済み。
`e1f4566c`のM6は18試行でMemory/smallのRSS中央値差101,412KiBが固定98,304KiBを超過してinvalidとなった。
Rust増分10,767,545Bとresident資源は既存予算内だが、4N Memoryは未実施であり全体性能未採用。
失敗の再送、予算や中央値規則の変更、pair差への読み替えは行わない。

全malloc環境設定を排除した同期対照ではmode間Anon差の符号が反転し、候補固有の正の差という予測は不支持。
その後、各mode一回だけ、同binaryでGNU mmap閾値だけを131072へ固定する探索を事前登録した。
warmup-first/measure-lastの全4端点でAnonは約314〜401MiB減り、同modeのresident集合は完全同一、Rust/File差は小さかった。
これはpolicy感度の証拠であり、正確なfreeheap所有量や性能合格にはしない。

最小修正候補はLinux/GNUの通常・profiling共通のApp前で、現arena2に`mallopt(M_MMAP_THRESHOLD,131072)`を追加すること。
codec early-returnと他OS/非GNUを保ち、両呼出結果を検査・記録する。診断envを製品条件にせず、main自身へ適用する。
FFI成功・各失敗の起動拒否を検証し、Help実レビュー、check/Clippy、独立source reviewの後に新sourceでgalleryと全24 M6をfresh実行する。
元の失敗・診断を保持し、性能の固定予算と未検証範囲を変えない。旧sourceのlive verifier依存3件だけは独立照合後に正規release済み。
