# 影スタイル 2D 化計画（床・壁接続維持） 2026-04-12

> **Archived 2026-07-13**: 実装と恒久ドキュメントへの反映が完了したため、設計判断の履歴として保存する。

## 問題

現在の 3D RtT 影は `DirectionalLight + shadow map` をそのまま使っており、
床・壁の接続自体は正しいが、見た目は自然寄りで 2D ゲーム風のデフォルメが足りない。

今回の本来目標は **Soul の影だけを別形状にすることではなく、床と壁の接続を守ったまま影の見た目全体を 2D ゲーム風にカスタムすること** である。

## 目標

- 床と壁で影の方向・接続・連続性が崩れない
- 既存の 3D shadow map 経路は維持する
- 影の見た目だけを 2D ゲーム風に寄せる
- `Soul` / 壁 / 地形が同じ光源条件で矛盾なく見える

## 非目標

- `GLB shadow proxy` を `blob mesh` に置き換えること自体を主目標にしない
- shadow map を廃止して decal / fake shadow に全面移行しない
- 床だけ、壁だけを別ルールで stylize しない

## ここまでの確定事項

### 0. 2026-04-14 方針再修正

receiver 側の `shadow_style` と world-space projector の両方を試したが、
初回実装ではユーザー観測に十分な変化が出なかった。

さらに `RtT composite + SoulMask` の add-on 影も probe で切り分けた結果、
**1 体ごとの Soul ではなく全 Soul の集約 mask を見ていたため、個別の外周フェードには使えない** ことが確定した。

また `SoulShadowMaterial` の prepass / shadow pass で caster 自体を外周 discard する案は、
半透明・ぼかしではなく「表示されない角度が増えるだけ」になり、要求に合わない。

したがって最終方針は、
**床 / 壁 receiver shader に Soul ごとの projected shadow を直接足す** 方式で固定する。

採用方針:

- 床 / 壁の既存 `DirectionalLight + shadow map` は維持する
- `SoulShadowProxy3d` の real caster も維持する
- Soul ごとの world-space projector 情報を shared material に流す
- terrain / wall shader で、その projector から **半透明・ぼかしつきの追加 shadow** を描く

この方式なら:

- 床と壁の接続は既存 shadow map が維持する
- `Soul` ごとの識別を失わない
- 透明度・ぼかしを receiver 側で連続量として扱える
- 追加コストは「少数 projector 配列更新 + receiver shader 小ループ」で済む

### 0.5. パフォーマンス制約

- 追加 pass は増やさない
- projector 数は固定上限（12）に制限する
- terrain / wall material の per-frame 更新は共有 handle のみ
- receiver shader は少数 projector のみをなめる
- `visual_test` や debug shader は恒久経路へ持ち込まない

### 0.75. なぜ projector 方式に戻すか

- `RtT composite + SoulMask` が不成立だった理由は「全 Soul 集約 mask」を使っていたからであり、`Soul` ごとの world-space projector が不成立だったわけではない
- 求められているのは「discard」ではなく「半透明・ぼかし」なので、receiver 側で連続量として扱える projector の方が要件に合う
- 床 / 壁に同じ projector を適用できるので、接続維持にも向いている
- したがって、**要求に最も素直でコストも軽い経路は receiver projector** と判断する

### 1. 2026-04-12 実装メモ

Stage 1 / Stage 2 の結果、採用経路は次で固定した。

- `TerrainSurfaceMaterial` / `TerrainSurfaceMaterialLod1Lite` / `TerrainSurfaceMaterialLod2`
- `SectionMaterial`
- いずれも `apply_pbr_lighting` の直後、`main_pass_post_lighting_processing` の前で共通 `shadow_style` を適用する
- shadow 判定は `lit/base_color` 比の近似ではなく、Bevy 0.18 の `shadows::fetch_directional_shadow(...)` を直接使用する

つまり本計画の mainline は、**既存 shadow map の受光結果を receiver 側で共通 stylize する** 方式である。
caster / proxy / `soul_shadow_prepass` は引き続き本筋ではない。

### 1. 接続を守る土台は既存 shadow map 経路

床と壁の接続が成立している理由は、両方が同じ light / same-space shadow を受けているからである。
したがって、本筋では **caster 置換より receiver 側のスタイル化** を優先する。

### 2. `blob caster` は補助実験としては成立

`visual_test` では既存 `GLB shadow proxy` と `blob` 候補を同条件で A/B 比較できる状態を作り、
少なくとも見た目を一致させる方向性は確認できた。

ただしこれは「caster 置換が可能か」の検証であり、
**本来目標である床・壁接続維持つきの 2D shadow style 実装そのものではない**。

### 3. 今後の主戦場は receiver projector 側

現状のコード上、床は `TerrainSurfaceMaterial` 系、壁は `SectionMaterial` 系で描画されている。
したがって、2D shadow style を本番実装するなら次のどちらかになる。

- 両マテリアルで同じ shadow stylize 関数を使う
- `Soul` の追加 shadow は projector で共通適用する

床・壁接続を最優先するなら、**床 / 壁の本影は同じ shadow map 規則を通し、Soul だけ同じ projector を追加する** のが最も安全。

## 実装方針

### 方針 A: 既存 shadow map を保持し、受け側の見た目だけを変える

最優先案。現在の連続 shadow を壊さず、床と壁の見た目を同じルールで stylize する。

候補:

- shadow の段階化
- shadow edge をやや硬くする
- 影色を寒色 / 暖色寄りに寄せる
- 接地付近だけ少し濃く見せる
- ラフスケッチ風のムラを影濃度に乗せる

### 方針 B: caster 側の抽象化は第 2 段階

もし receiver 側の style だけでは `Soul` の影シルエットが still too realistic なら、
その時点で初めて `SoulShadowProxy3d` の shape abstraction を別タスクとして導入する。

これは本計画の補助軸であり、主軸ではない。

## 実装ステージ

### Stage 0: ベースライン固定

- 現在の runtime は既存 `GLB shadow proxy` 経路を維持する
- `visual_test` の A/B 比較経路は補助検証用として維持する
- この段階では本番の caster 経路は変えない

完了条件:

- `cargo check --workspace` が通る
- 本編の見え方が現在の正常状態から変わらない

### Stage 1: shadow 受け側の経路確認

対象:

- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `crates/hw_visual/src/material/section_material.rs`
- `assets/shaders/terrain_surface_material*.wgsl`
- `assets/shaders/section_material.wgsl`

やること:

- 地形側で shadow の寄与をどこで最終色へ反映しているか確認
- 壁側で shadow の寄与をどこで最終色へ反映しているか確認
- 両方に共通の stylize 挿入点を取れるか確認

重要:

- 推測で「shadow term がここにあるはず」と決め打ちしない
- Bevy 0.18 の shader 経路と、repo 内の実 shader を一次情報として確認する

完了条件:

- 「共通関数でいける」か「合成段でやるべき」かを明文化できる

### Stage 2: `Soul` projector パラメータ設計

候補パラメータ:

- `shadow_steps`
- `shadow_softness`
- `shadow_tint`
- `shadow_noise_amount`
- `ground_contact_boost`
- `radius`
- `feather`
- `strength`
- `forward_extent`

要件:

- 既存 terrain / wall shared material に projector 配列を渡す
- projector は `world center + radius + forward extent` で表す
- per-frame 更新は共有 material handle のみ

完了条件:

- uniform / material ext / shader binding の入れ先が決まる

### Stage 3: receiver shader への `Soul` projected shadow 実装

優先順位:

1. 既存の床 / 壁 shadow map はそのまま残す
2. Soul ごとの projector 配列を terrain / wall material に流す
3. 追加 shadow は shadow map とは独立に darken する
4. blur / opacity は receiver 側の式だけで詰める

ルール:

- 地形だけ先に強く変えない
- 壁だけ先に強く変えない
- 常に床と壁を同じシーンで確認する
- composite 側の集約 mask は再利用しない
- floor / wall には同じ projector ロジックを入れる

完了条件:

- 床から壁へ影が接続したまま、自然 shadow より 2D 寄りに見える
- `Soul` 影が中心ほど濃く、外側ほど薄くなる
- 床 / 壁の接続は既存 shadow map のまま崩れない

### Stage 4: 実シーンでの接続確認

確認シーン:

- 壁際に `Soul` が立つケース
- 床から壁へ斜めに影が乗るケース
- 地形境界近傍
- 仮設壁 / 本設壁の両方

見る点:

- 床と壁の境界で影の濃さが急に切り替わらないか
- 地形だけ shadow style が浮いて見えないか
- 壁だけ硬すぎる / 暗すぎる破綻がないか

### Stage 5: 必要なら Soul caster 抽象化を別タスク化

ここで初めて判断する:

- receiver style + projector だけで十分か
- `Soul` だけ silhouette をさらに単純化したいか

必要なら別 plan に分離する。
`blob` はこの段階で初めて mainline 候補になる。

## 変更対象ファイル（本筋）

### 調査・本実装候補

- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `crates/hw_visual/src/material/section_material.rs`
- `assets/shaders/terrain_surface_material.wgsl`
- `assets/shaders/terrain_surface_material_lod1_lite.wgsl`
- `assets/shaders/terrain_surface_material_lod2.wgsl`
- `assets/shaders/section_material.wgsl`
- `assets/shaders/shadow_style.wgsl`
- `crates/bevy_app/src/systems/visual/soul_shadow_projector.rs`
- `crates/bevy_app/src/plugins/visual.rs`
- `crates/hw_visual/src/material/terrain_surface_material.rs`
- `crates/hw_visual/src/material/section_material.rs`

### 補助確認

- `docs/architecture.md`
- `crates/visual_test/src/*`

## 変更対象ファイル（今回の本筋ではない）

以下は必要になるまで触らない:

- `crates/bevy_app/src/entities/damned_soul/spawn.rs`
- `crates/bevy_app/src/systems/visual/character_proxy_3d.rs`
- `crates/bevy_app/src/plugins/startup/visual_handles.rs`
- `assets/shaders/soul_shadow_prepass.wgsl`
- `crates/hw_visual/src/material/soul_shadow_material.rs`

これらは caster 置換や shadow proxy 最適化の話であり、本計画の主軸ではない。

## リスク

### 1. 地形と壁で shader の shadow 受け方が非対称

対策:

- Stage 1 で挿入点を先に確定する
- 共通化が難しい場合は合成段への移動を判断する

### 2. projector が既存 shadow と二重に見えて強すぎる

対策:

- strength を projector 側だけで独立制御する
- 既存 shadow style と掛け算せず、追加 shadow として別 darken にする

### 3. projector が広すぎて他 Soul と干渉する

対策:

- projector 数を近傍上位に制限する
- 半径と forward extent を抑える

### 4. 地形 LOD ごとに見え方がズレる

対策:

- LOD1 / LOD1-lite / LOD2 の 3 本を同時に確認する
- パラメータ命名と意味を共通化する

### 3. projector 数上限で遠方 Soul に効果が乗らない

対策:

- camera 近傍 / 画面内優先で projector を詰める
- それでも不足する場合のみ上限値を増やす

### 4. `Soul` の silhouette だけが still too realistic

対策:

- 本計画ではそこで止める
- 必要なら caster abstraction を別計画に切り出す

## 検証

最低限:

- `cargo check --workspace`

目視確認:

- 本編で床と壁の接続を見る
- `visual_test` は補助として使うが、最終判断は本編シーンで行う

## 成功条件

- 床と壁の影接続が維持される
- 影の見た目が 2D ゲーム風に寄る
- `Soul` / 壁 / 地形の光源解釈が破綻しない
- `blob` 置換の有無に関係なく、本来目標を達成できる

## メモ

`blob` A/B 比較経路は捨てない。  
ただし役割は **本筋の代替案の検証** であり、今後の mainline 実装判断を補助するためのものと位置付ける。
