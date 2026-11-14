# 建設設備を面塗り主体の描画へ変更する

## メタ情報

| 項目 | 値 |
| --- | --- |
| 計画ID | building-illustrated-shading-2026-11-14 |
| ステータス | In Progress |
| 作成日 / 最終更新日 | 2026-11-14 |
| 作成者 | Codex |
| 関連Issue | TAK-14 導入後のシェーダーフィードバック |

## 1. 目的

g3の材色変更でも3D感が強いという利用者の観測を制約にする。丸みを連続的な陰影で示すPBRから、
描いた色面を主体にした平面的な挿絵へ変更する。材色の再調整では解決済みと扱わない。

## 2. スコープ

非橋設備の構造材質とDoorのopt-in shaderを変更する。骨・布・錆・木の色面、実状態・emission、
alpha・depth・施工clip・光と遮蔽の成立を保つ。Wall/Floor/Bridge、foreground Sprite、
ゲーム論理・形状・manifest世代・asset authorityは変更しない。

## 3. 現状とギャップ

structural shaderはPBR照明と後段shadow styleを使う。Tankだけ法線0.82混合をするが、
他設備では面の幾何法線による丸み・箱の明暗がそのまま残る。compositeはtextureの転送だけ。
未使用のindoor light field fragment readは起動失敗の履歴があり、今回再有効化しない。

## 4. 方針と比較基点

既存uniformのwを明示的なillustration opt-inへ変更し、ABIを増やさない。
albedoと照明参照を分離し、白色・無反射・上向き面の実照明を少数の明度段階で受ける。
幾何法線の滑らかな陰影とspecularを色面から取り除く。実影、露出、発光、既存post処理を保つ。
旧PBR経路は非対象材質へ残す。sourceの制御とnative GPU実画面の両方で境界を確認する。
同目的branch codex/tak14-non-bridge-releaseを再利用し、基点c520c2fa6f1ac31d20b4d84054db042e40295287。
主担当が編集・検証・Gitを所有、独立reviewはread-only。公開・push/mergeは行わない。

## 5. マイルストーン

- [ ] albedo-first照明分離・対象限定、発光とalpha/depth/clipの保持を実装する。
- [ ] CPU側適用・除外と状態継承、shader契約を検証する。
- [ ] 同assetのnormal/far実画面を旧PBR画像と比較し、立体陰影が減る仮説を一度で判定する。
- [ ] 採用sourceで必要なnative性能、Help実レビュー、CI/Clippy、独立reviewを完了する。
- [ ] 恒久仕様と画像へ集約し、一時計画/索引/不要出力を整理する。

## 6. リスク・検証

陰影を減らして開口や状態を失うリスク、暗所の発光化、段階境界のちらつきを通常/遠景で確認する。
無変化なら同系統の値調整を続けず、幾何投影/画像方式を再評価する。
shader/APIはローカルBevy0.19一次sourceと既存実装で確認する。
dev.py check、cargo clippy --workspace --all-targets -- -D warnings、変更別CIをprimary guard経由で実行する。
Help Skillとnative Skillを適用し、validationのretain/plan/seal/finalize/checkを使用する。
旧終了jobは再開せず、新subjectはfresh jobへ拘束する。実画面の最新提示・修正用cacheはフィードバック中保持する。
閉じた比較のrawは恒久仕様へ結果を集約後に整理し、原本と採用assets/旧世代は保全する。
