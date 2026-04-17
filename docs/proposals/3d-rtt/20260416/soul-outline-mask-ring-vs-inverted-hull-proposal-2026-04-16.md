# Soul 外周強調方式比較提案

## メタ情報

| 項目 | 値 |
| --- | --- |
| ドキュメントID | `soul-outline-mask-ring-vs-inverted-hull-proposal-2026-04-16` |
| ステータス | `Draft` |
| 作成日 | `2026-04-16` |
| 最終更新日 | `2026-04-16` |
| 作成者 | `Codex (GPT-5)` |
| 関連計画 | `TBD` |
| 関連Issue/PR | `N/A` |
| 関連提案 | `docs/proposals/3d-rtt/20260316/outline-rendering-proposal-2026-03-16.md` |
| 関連提案 | `docs/proposals/3d-rtt/20260317/character-3d-rendering-proposal-2026-03-16.md` |

## 1. 背景と問題

- 現状:
  - Soul body は `CharacterMaterial` で描画し、`assets/shaders/character_material.wgsl` の `dot(curved_n, v)` ベース rim で白を混ぜている。
  - Soul silhouette は別系統で `SoulMaskProxy3d` を `LAYER_3D_SOUL_MASK` に流し、`RttCompositeMaterial` が最終合成時に少し膨らませて丸めている。
- 問題:
  - body shader の rim は外周だけでなく内部の曲面にも乗るため、「外周面だけを強調する」見え方になりにくい。
  - composite 側は現在 silhouette を少し膨らませるだけで、明示的な外周リングや線色の制御を持っていない。
- なぜ今やるか:
  - Soul の 3D GLB 表示はすでに RtT + mask の常設経路を持っており、今の責務分担のままでも外周強調の改善余地が大きい。
  - 先に比較提案を固めておかないと、body shader の rim 調整と別パス追加を場当たりで混在させやすい。

## 2. 目的（Goals）

- Soul の見た目を「内部面のハイライト」ではなく「画面上の外周強調」に寄せる。
- 既存の RtT / soul mask 構成を前提に、実装責務を明確にした比較を残す。
- 実装前に、性能・保守性・見た目の安定性の観点で採用候補を絞る。

## 3. 非目的（Non-Goals）

- 建築物や地形まで含む汎用アウトライン基盤の設計。
- Soul face atlas や表情切替ロジックの刷新。
- Soul の影表現、section cut、projected shadow の刷新。
- 1 回の提案で最終アートパラメータを固定すること。

## 4. 提案内容（概要）

- 一言要約:
  - `案1: soul mask ring` と `案3: inverted hull` を比較し、**本線は案1、案3は高コストな実験案**とする。
- 主要な変更点:
  - 案1は `RttCompositeMaterial` に外周リング生成を追加する。
  - 案3は Soul 用の追加描画パスを導入し、膨張メッシュの背面を単色描画する。
- 期待される効果:
  - 外周だけを強調しつつ、body shader 内部の白飛びを減らせる。
  - 実装前に「見た目は強いが重い案」と「既存構成に自然に乗る案」を分けて評価できる。

## 5. 詳細設計

### 5.1 現状の制約

- `CharacterMaterial` は Soul body の立体感と幽体感を 1 パスで担っている。
- `RttCompositeMaterial` はすでに `scene_texture` と `soul_mask_texture` を同時に参照できる。
- `SoulMaskProxy3d` は本体と同じ GLB を別レイヤーへ複製スポーンする常設経路を持つ。
- `visual_test` には `ghost_alpha` / `rim_strength` / `posterize_steps` の即時調整 UI があるため、比較検証の足場を追加しやすい。

### 5.2 比較対象

#### 案1. Soul mask ring を composite 側で生成する

- 方式:
  - `RttCompositeMaterial` で `rounded_mask` と `center_mask` の差分から外周帯を作る。
  - その帯にだけ `outline_color` と `outline_strength` を掛ける。
  - 必要なら現行の「丸め」処理は残し、輪郭色付けだけを追加する。
- 実装責務:
  - `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
  - `assets/shaders/rtt_composite_material.wgsl`
  - `docs/architecture.md`
  - `docs/visual_test.md`
- 想定 uniform 追加:
  - `outline_width_px`
  - `outline_feather`
  - `outline_strength`
  - `outline_color`
- shader 概念:

```wgsl
let outer_mask = rounded_mask;
let inner_mask = smoothstep(0.10, 0.55, center_mask);
let outline_band = clamp(outer_mask - inner_mask, 0.0, 1.0);
let outlined_rgb = mix(composed_rgb, material.outline_color.rgb, outline_band * material.outline_strength);
```

- 長所:
  - 既存の soul mask RtT をそのまま再利用できる。
  - 「画面上のシルエット外周」を直接扱うため、内部曲面へ漏れにくい。
  - Soul 本体の GLB 子孫や material 差し替えを増やさずに済む。
- 短所:
  - screen-space 処理なので、接近した複数 Soul の mask が重なると輪郭が連結して見える可能性がある。
  - 断面や他オブジェクトとの前後関係で「完全な線」より halo に近い見え方になる。

#### 案3. Inverted hull / shell outline を 3D で追加する

- 方式:
  - Soul 本体とは別に outline 用 proxy を 1 体追加する。
  - 頂点法線方向へ少し膨張したメッシュを背面描画し、単色の outline material で出す。
  - 必要なら `CullMode::Front` 相当の設定、または vertex shader 側膨張を使う。
- 実装責務:
  - Soul spawn / cleanup / sync 系
  - outline 用 material と shader
  - 追加 RenderLayer または既存 `LAYER_3D` 合流設計
  - section / mask / shadow との干渉整理
- 想定変更対象:
  - `crates/bevy_app/src/systems/visual/...` の Soul proxy 管理
  - `crates/hw_visual/src/material/` に新規 outline material
  - `assets/shaders/` に新規 outline shader
  - `docs/architecture.md`
  - `docs/rendering-performance.md`
- shader / render 概念:

```text
Soul GLB本体
  + SoulOutlineProxy3d
      same GLB
      inflate along normal
      draw backfaces only
      flat outline color
```

- 長所:
  - screen-space ではなくジオメトリ由来なので、側面や斜め視点でも「線」として見えやすい。
  - Soul 同士が近くても silhouette が自動連結しにくい。
  - 将来、線幅の方向依存や揺らぎを 3D 側で持たせやすい。
- 短所:
  - Soul ごとに追加の draw / scene proxy / 同期対象が必要になる。
  - GLB 子孫構造、face mesh、section cut、shadow 設定との干渉面が多い。
  - 透明・不透明混在や前面 face の抜け方次第で z-fight 風の破綻が出やすい。

### 5.3 変更対象（想定）

- 案1:
  - `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
  - `assets/shaders/rtt_composite_material.wgsl`
  - `crates/visual_test/src/*` のうち outline パラメータ露出箇所
  - `docs/architecture.md`
  - `docs/visual_test.md`
- 案3:
  - `crates/bevy_app/src/systems/visual/soul_*`
  - `crates/hw_visual/src/material/*` 新規 material
  - `assets/shaders/*` 新規 outline shader
  - `docs/architecture.md`
  - `docs/rendering-performance.md`

### 5.4 データ/コンポーネント/API 変更

- 案1 追加:
  - `RttCompositeParams` に outline 系 uniform を追加。
- 案3 追加:
  - `SoulOutlineProxy3d` 相当の marker / config component。
  - outline 用 material asset と render layer 運用。
- 変更:
  - どちらの案でも `CharacterMaterial` の `rim_strength` は「主輪郭」ではなく補助表現へ再定義する可能性が高い。
- 削除:
  - 本提案段階ではなし。

## 6. 代替案と比較

| 案 | 採否 | 理由 |
| --- | --- | --- |
| 案1: Soul mask ring | 採用候補 | 既存 RtT / mask 構成に自然に乗り、外周だけを画面空間で扱える。変更面が狭く、比較的安全。 |
| 案3: Inverted hull | 不採用候補（研究用） | 見た目の自由度は高いが、Soul 専用 proxy と追加 draw が必要で、現行構成には重い。 |

### 6.1 比較表

| 観点 | 案1: Soul mask ring | 案3: Inverted hull |
| --- | --- | --- |
| 外周だけを拾う精度 | 高い。画面上 silhouette そのものを使う | 高い。ジオメトリ由来の線を作れる |
| 内部曲面への漏れ | 小さい | 小さい |
| 近接 Soul 同士の分離 | 弱い。mask 重なりで連結しうる | 強い。個別メッシュとして維持しやすい |
| 実装量 | 小 | 大 |
| draw call / proxy 増加 | ほぼなし | Soul 数に比例して増える |
| デバッグ容易性 | 高い。composite 1 箇所で追える | 低い。spawn / sync / shader / culling を跨ぐ |
| 既存アーキテクチャとの整合 | 高い | 中 |
| 断面・shadow との干渉 | 小 | 中〜大 |
| visual_test での比較のしやすさ | 高い | 中 |

### 6.2 採用判断

- 本線:
  - **案1を採用候補とする。**
- 判断理由:
  - すでに `RttCompositeMaterial` が Soul 専用 mask を持っており、責務が一致している。
  - 変更箇所が狭く、`visual_test` でパラメータ比較しやすい。
  - 現在の問題は「外周面だけを強調したい」であり、まず必要なのは追加ジオメトリではなく輪郭帯の抽出である。
- 案3の位置づけ:
  - Soul をより「インク線」に近く見せたい、または side view で halo 感が許容できない場合に再評価する。
  - 実装の前に、proxy 数増加と culling の整理を伴う別提案または詳細計画が必要。

## 7. 影響範囲

- ゲーム挙動:
  - Soul の視覚表現のみ。AI・タスク・移動には影響しない。
- パフォーマンス:
  - 案1は composite shader の追加演算分のみ。
  - 案3は Soul 数比例の描画負荷・proxy 管理負荷を増やす。
- UI/UX:
  - 視認性向上。ズーム時の Soul 判読性が上がる可能性がある。
- セーブ互換:
  - なし。
- 既存ドキュメント更新:
  - 実装する場合は `docs/architecture.md` と `docs/visual_test.md` は必須。
  - 案3を採る場合は `docs/rendering-performance.md` も必須。

## 8. リスクと対策

| リスク | 影響 | 対策 |
| --- | --- | --- |
| 案1で近接 Soul の outline が一体化して見える | 群集で輪郭が濁る | `outline_strength` を抑え、低解像度時は width を縮める。必要なら将来 object-id 系の分離を検討する。 |
| 案1で halo が太りすぎる | ぼやけて見える | `outline_band` を `outer - inner` で細く限定し、既存丸めと強調を分離する。 |
| 案3で z-fight / face 抜けが出る | 破綻が目立つ | front cull・深度・face mesh 除外を先に設計し、body のみ対象に限定する。 |
| 案3で proxy 管理が複雑化する | 保守コスト増 | 追加前に spawn / cleanup / sync の責務表を設計する。 |

## 9. 検証計画

- `visual_test` に outline 幅・強度の比較 UI を追加する前提で検証する。
- 手動確認シナリオ:
  - TopDown で 1 体表示し、外周のみが強調されるか確認。
  - 複数 Soul を近接配置し、輪郭の連結や破綻を確認。
  - `V` キーで視点を変え、斜め・側面での見え方を確認。
  - 低ズーム時に outline が太りすぎないか確認。
- 計測/ログ確認:
  - 案1: 追加 draw が増えていないことを確認。
  - 案3: Soul 数増加時の draw / proxy 数増加を確認。

## 10. ロールアウト/ロールバック

- 導入手順:
  - まず案1を `visual_test` で比較実装し、良ければ本編へ移植する。
- 段階導入の有無:
  - あり。`visual_test` -> 本編。
- 問題発生時の戻し方:
  - 案1は `RttCompositeMaterial` の outline uniform を 0 に戻せばよい。
  - 案3は outline proxy spawn を止めるだけでは不十分で、追加 layer / material / sync 経路ごと戻す必要がある。

## 11. 未解決事項（Open Questions）

- [ ] 案1で「外周リング」と「既存の silhouette 丸め」を同時に使うか、外周リングのみに寄せるか。
- [ ] 案1の outline 色を白系にするか、暗青系にするか。
- [ ] 案3を将来再評価するなら、body のみ対象に限定して face mesh を除外する方針でよいか。
- [ ] 群集シーンでの近接 outline 連結が、実際のアート方向として許容されるか。

## 12. AI引継ぎメモ（最重要）

### 現在地

- 進捗: `100%`
- 直近で完了したこと:
  - 案1と案3の比較提案を文書化した。
  - 採用候補を案1に寄せる判断理由を整理した。
- 現在のブランチ/前提:
  - docs-only 更新。コード未変更。

### 次のAIが最初にやること

1. `assets/shaders/rtt_composite_material.wgsl` の `rounded_mask` / `center_mask` を用いた outline band 式を PoC 実装する。
2. `visual_test` に outline 幅・強度調整 UI を追加する。
3. 近接 Soul 2〜6 体で halo 連結が許容範囲か確認する。

### ブロッカー/注意点

- 案3は比較対象としては有効だが、実装へ進むなら別途詳細設計が必要。
- 現行の問題は body shader の rim 調整だけでは解決しにくい。外周強調の責務を composite 側へ寄せる前提で考えること。

### 参照必須ファイル

- `docs/architecture.md`
- `docs/visual_test.md`
- `crates/bevy_app/src/plugins/startup/rtt_composite.rs`
- `assets/shaders/rtt_composite_material.wgsl`
- `assets/shaders/character_material.wgsl`

### 完了条件（Definition of Done）

- [x] 提案内容がレビュー可能な粒度で記述されている
- [x] リスク・影響範囲・検証計画が埋まっている
- [x] 実装へ進む場合の次アクションが明記されている

## 13. 更新履歴

| 日付 | 変更者 | 内容 |
| --- | --- | --- |
| `2026-04-16` | `Codex (GPT-5)` | 初版作成 |
