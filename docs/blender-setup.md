# Blender AI モデリング環境

## 1. 目的と境界

この環境の標準経路は、Codex から MCP 経由で Blender のシーンを編集し、
`.blend` と GLB を品質ゲート付きで管理することです。

AI モデリングは次の二つを分離します。

1. **AI 支援編集**: `blender-ai` で明示的に MCP セッションを開始し、オブジェクト、
   transform、material、render、undo/redo、staging 保存を操作する。
2. **text/image-to-3D 生成**: 外部サービスまたは別GPU端末の出力を
   `staging/imports/` に隔離し、Blender で修正してから同じ品質ゲートへ流す。

この端末は Intel Arc 統合 GPU のため、VRAM を多く要求するローカル3D生成モデルは
標準構成に含めません。生成物を正本へ直接書くクラウドアドオンも自動導入しません。
必要になった場合は、ライセンスと利用規約を確認したサービスまたはGPU実行環境から
`staging/imports/` へ受け入れます。

## 2. 固定した実測ベースライン

| 要素 | 固定値 |
|---|---|
| Blender | `5.1.1`、Fedora Flatpak `org.blender.Blender` |
| Flatpak commit | `a55abdc01ce63065cc5c61bb14e83b89820ffe540bae514370a0b20cade4b24e` |
| Blender MCP | upstream `v0.1.3` / commit `7eed33edf4aca2ab0ca84a6da27321f89f68b504` + project hardening patch |
| Python MCP SDK | `1.29.0`（`mcp<2`） |
| Khronos validator | `gltf-validator 2.0.0-dev.3.10` |
| Asset root | `~/Sync/hell-workers-assets` |

`2026-08-01`のユーザー判断により、旧 Blender 端末からasset、設定、addon、presetを
引き継がず、この表を現端末の新規canonical authoring baselineとします。
`source/`と`exports/`は空から開始し、stagingで新規制作・検証・目視承認した成果物だけを
正本へ昇格します。production runtimeと`visual_test`はP08以降Soul GLBを読み込まず、shared-pool billboardを使います。ローカルに残る旧GLBをBlenderへ逆変換して正本扱いしません。

## 3. ディレクトリ

```text
~/Sync/hell-workers-assets/
├── source/
│   ├── blender/       # 承認済みの正本 .blend
│   ├── generated/     # 承認済み生成原本
│   └── references/    # 参照画像、prompt等
├── staging/
│   ├── imports/       # 未信頼のAI生成物・変換コピー
│   ├── blend/         # AI編集セッションの保存先
│   ├── exports/       # 品質確認前のGLB
│   ├── renders/       # 目視確認用
│   ├── reports/       # scene/Khronos JSON
│   └── snapshots/     # アセット作業用snapshotのみ
├── exports/
│   ├── models/        # 承認済み配布候補
│   ├── textures/
│   └── audio/
├── manifests/
└── licenses/
```

`staging/` は隔離領域であり正本ではありません。自動化は `source/`、
canonical `exports/`、リポジトリ内 `assets/` へ直接書きません。

Blender の `userpref.blend` バックアップは秘密を含む可能性があるため、
同期rootではなく次のローカル領域に `0600` で保存します。

```text
~/.var/app/org.blender.Blender/config/blender/5.1/config/backups/hell-workers/
```

## 4. 起動

通常編集では MCP を起動しません。

```bash
blender
```

AI 編集を行うときだけ、次を使います。

```bash
blender-ai
```

`blender-ai` は hardened addon を明示起動し、`127.0.0.1:9876` で待ち受けます。
終了時にはポートも閉じます。認証のない localhost プロトコルなので、
port-forward、外部公開、共有端末での常時起動は禁止です。

Codex は project-scoped `.codex/config.toml` を読みます。設定追加後の最初の利用時は
リポジトリを trusted project として開き直し、Codex セッションも再起動してください。

```bash
cd ~/projects/hell-workers
codex mcp list
```

表示名 `blender` が `enabled` なら登録済みです。Codex 側では読み取りだけを自動許可し、
作成・削除・transform・material・render・保存・undo/redo は確認付きです。

## 5. 安全境界

`tools/blender_ai_workflow/bin/blender-safe` は Flatpak の host filesystem 権限を外し、
次だけを見せます。

- asset root: read/write
- repository: read-only
- pinned MCP vendor: read-only
- embedded `.blend` Python: `--disable-autoexec`

addon はさらに次を強制します。

- bridge bind は `127.0.0.1` のみ
- 自動起動は無効。`blender-ai` セッションでだけ開始
- file access は `staging/` のみ
- `.blend` 保存は `staging/blend/` のみ
- inline/file Python、async job、OBJ/FBX、直接 glTF export は whitelist 外
- MCP server の Python/headless transport も環境変数で既定拒否

validation、export、smoke は `--factory-startup` とネットワーク分離で実行し、
ユーザーaddonやライブbridgeを読みません。

## 6. 日常ワークフロー

```text
AI/外部生成
  → staging/imports
  → blender-ai で編集
  → staging/blend に保存
  → scene validator
  → staging/exports にGLB
  → Khronos validator
  → render・report・license・provenanceを人が確認
  → 明示承認後だけ canonical exports へ昇格
  → sync_external_assets.py --dry-run
  → repo assets 反映
  → game/visual_test 目視
```

例:

```bash
ASSET_ROOT="${HELL_WORKERS_ASSET_ROOT:-$HOME/Sync/hell-workers-assets}"

tools/blender_ai_workflow/bin/validate-blend \
  "$ASSET_ROOT/staging/blend/model.blend" \
  model.scene.json \
  50000

tools/blender_ai_workflow/bin/export-staging-glb \
  "$ASSET_ROOT/staging/blend/model.blend" \
  model.glb \
  50000

python3 scripts/sync_external_assets.py \
  --source "$ASSET_ROOT/exports" \
  --dry-run
```

`export-staging-glb` は scene gate に合格した場合だけ Blender export を実行し、
続けて公式 Khronos validator を実行します。直接 MCP export は禁止しているため、
この経路を迂回できません。

production Wallは1 tile単位の6 collectionを`create-wall-production-scene`で作ります。Blender 5.1.1のglTF
operatorにはglobal scale propertyがないため、正式exportは`--geometry-scale 32 --materials-mode placeholder`を
明示し、検証後のin-memory mesh copyへだけ32倍をbakeします。保存済み`.blend`と既存exportの既定挙動は変更しません。
`render-wall-reference-board`はM0と同じOCIO陽性configを強制し、59.036° Orthographicで6 familyを描画します。
OCIO fallback時はreference reportをpassにしません。

### 本設Wallの面別UV

`create-wall-production-scene`の`wall-face-uv-v4`は、側面を輪郭辺の接線方向×高さで展開する。
辺の長さを0〜1へ正規化せず、同じ表面種別の縦横texel密度を揃える。石面は上下段でも
高さ座標をリセットしない。上下面はatlas右下の専用256px断面textureを使い、
外壁の石積みを断面へ貼らない。厚さ・接続port・三角形数・単一primitive / materialの契約は変えない。
下半分をedge番号に応じてrust/purpleへ切り替える処理は廃止し、全側面をstoneへ統一する。
等倍で線や点へ潰れる鉄板部品・留め具・黒い輪郭を避け、1 tileあたり約3段の大きな石面と
弱い目地で材質を示す。上下のmesh分割自体は旧geometryとの一致のため保持する。

新規出力は旧geometry gateに加え、`wall_surface_uv.py <output.glb> --family <family> --report <staging-report>`
でUV0を再検証する。Blender→glTFの軸・V反転を戻して全三角形を照合し、側面の線状UV・縦横比の
引き伸ばし・装飾を貼った断面・旧rust/purple側面を拒否する。このprofileは旧releaseや木製型枠には遡及適用しない。
元textureを変更する場合は内部色領域の再確認も必要となる。

石面と断面用画像は画像生成で用意する。断面は暗い石色の広い不規則な内部模様とし、
細粒のノイズ・黒い亀裂・レンガ列・鉄板の目地・発光・タイルごとの枠を描かない。外壁との模様の差で境界を示し、
外周線や欠けの追加geometryで壁厚を増やさない。新しい隔離`HELL_WORKERS_ASSET_ROOT`で
`pack_wall_core_atlas.py --source-dir <元textureディレクトリ> --core-art <断面画像> --stone-art <石面画像>`を実行すると、
ImageMagickで縮小・配置する。
image座標`(702,750)`から260×260の範囲へ、256px画像と2pxのfilter余白を収める。
石面は`(30,62)`から612×612の範囲へ608px画像と2px余白を収め、両軸のpixel中心間607pxをUVへ使う。
albedo/emissiveとも2領域外の全画素不変を検査し、石面・断面のemissiveは完全な黒とする。
使わなくなった装飾領域は画像内に残すが、本設UVから参照しない。既存出力へは上書きしない。
`core-atlas.json`は`pack_rects`、2枚の`artwork`原図・元atlas・出力hashと面別色統計を記録する。scene生成はprofileと出力hashの
一致を要求し、旧atlasへ新UVだけを適用する失敗を拒否する。PillowとImageMagickが必要。

断面は側面の石（目地を除く）の代表色へ合わせ、茶色く明るい別領域を使わない。
Blenderの確認用石材はruntimeの`make_topdown_structural_material`に合わせてroughness=1、
metallic=0、specular=0とし、ゲームに存在しない金属反射で上面の色を変えない。
これは反射特性を揃える契約で、Blenderとゲームの照明全体や最終画素の一致を主張しない。
`render-wall-reference-board --neutral-light`は白色照明の色確認用画像を別の`-neutral`名で作る。
既定の紫rim画像は維持し、reportの`lighting_profile`で区別する。各familyの実際の材質設定も
reportへ記録し、白色照明の画像をゲーム内証拠へ流用しない。

Wall boardの投影は`wall-preview-topdown-v1`。角度59.036°に加え、runtimeのRtT合成と同じ
`hypot(150,90)/150=1.16619038`の縦補正をpixel aspectで再現する。実cameraの6基準点を
`screen_up=y+.6*z`の解析値へ0.01px以内で照合し、全Wall頂点のcanvas内収容も確認する。
32×32 wu grid、EW/NS同倍率標本、3連結標本、厚9.6／高32の寸法labelは確認sceneにだけ追加し、
保存済みblendやGLBへ含めない。`--review-scale detail`は5px/wu、`standard`は1px/wu、
`farthest`はcamera scale=5相当の0.2px/wuで別名PNGを直接renderする。
これらは静止したBlender比較であり、ゲームのDPI、照明、AA、時間方向のちらつき検証を代替しない。

参照boardは実行root内のalbedo / emissiveへ画像参照を再接続し、そのhashをreportへ記録する。
コピーした旧原本との同条件比較でも保存済みblendを書き換えない。Blender boardだけでゲーム内の
承認やreleaseを成立させず、変更した完成Wallには新たなアート承認と世代封印を必要とする。
既存の「本設8 file不変」で承認された型枠追加manifestへ新UVのGLBを混ぜない。

本設surfaceの承認前確認には`provision_wall_surface_preview.py --repo <clean-primary>
--generation <新番号> --destination <隔離root/staging/validation/新ディレクトリ>`を使う。
本番schema-2 locator/receiptと15 coreの実bytesを検証し、本設6 meshの形状・法線不変、UV-v4、
export hash、atlas保護画素を確認する。型枠7 fileは本番bytesのまま保持し、新しい本設8 fileと共に
`surface_art_preview` manifestへ封印する。出力は新規の隔離先に限り、`authority=art_preview`、
`receipt=null`のruntime projectionだけを作る。旧世代のアート承認・本番反映権限は引き継がない。
この15-file viewをclean validation worktreeへ用意した後、nativeスキルの
`wall_art_acceptance.py plan --candidate --art-preview --completed-preview --repo <worktree> --adapter Intel`
で返されたdirect kitty commandを実行する。`--zoom farthest`は別の単独preview jobにする。
profileは`wall-surface-art-preview-v1-standard` / `wall-surface-art-preview-v1-farthest`。
最遠の単独previewでも、下位perf runnerが要求する`--wall-art-matrix`と`HW_WALL_ART_MATRIX=1`を
対で渡す。これは画質・zoom選択の許可であり、orchestratorのcase数を9件へ増やす意味ではない。
撮影・再検証ともcompleted phaseと候補identityを照合し、型枠の画像を本設の証拠へ流用しない。
perf設定入口も`wall_actual_window_phase_matches`でcompleted/provisionalに限定し、
ArtPreviewのmixed phase、phase未指定、actual-windowなしは拒否する。
既存型枠previewの既定動作は不変。いずれもアート承認・正式受入・性能baseline・昇格の証拠ではない。

### 壁・床以外の設備の無地制作（M0、runtime未接続）

`building-art-v1.contract.json`は全10種の論理shape、9新setのexact role、既存Door g7、
preview consumerと原点／単位を記録する。`check_building_art_contract.py`はRustの
`BuildingType::ALL`・shape定数・kindへの割当・中心・anchorとconsumer入口をread-onlyで照合する。
`--check-images`は砂・骨item iconと猫車2画像の実bytesも照合する。CIでは外部runtime mirrorを要求しない。

Tank / MudMixerの`building-*-v1.geometry.json`は**clay_draft**であり、`final_art=null`。
高さ・triangle上限・canvasは試作検査用の値で、ゲーム内無地判定後の最終freezeではない。
残る群の美術数値、`.buildingset` loader、native設備profile、release経路は未実装。

```bash
python3 tools/blender_ai_workflow/scripts/check_building_art_contract.py --check-images
python3 tools/blender_ai_workflow/scripts/build_building_clay.py build Tank --name tank-clay-review-001
python3 tools/blender_ai_workflow/scripts/build_building_clay.py build MudMixer --name mixer-clay-review-001
python3 tools/blender_ai_workflow/scripts/build_building_clay.py verify Tank --name tank-clay-review-001
```

`build`は新しい名前だけを受け付け、既存fileを上書きしない。Blenderはnetworkなし・factory-startup・
autoexec無効で動く。原本は`staging/blend/<name>.blend`、neutral albedo・状態PNG・role GLBは
`staging/exports/building-clay/<name>/`、制作reportは`staging/reports/<name>.json`、
role別export / Khronos / bundle reportは`staging/reports/building-clay/<name>/`へ出る。
canonicalとrepo assetsは一切変更しない。失敗した名前での再試行は拒否し、原因を直して新名を使う。

原本のrole objectは全てidentityで保存する。既存`export-staging-glb`のexact collection選択と
`--geometry-scale 32 --materials-mode placeholder`を通し、node transformを使わず頂点をworld unitへ変換する。
水面・rotorの配置はfixtureのworld-unit translationを使い、原本のobject translationへ焼き込まない。
後段validatorはGLB bytesの単一scene/node/mesh/primitive、identity、属性型・数、法線長、
非退化geometry/UV、bounds、triangle上限、skin/morph/animation/画像の不在を確認する。
`verify`は原本・script・fixture・PNG・GLBのhashを照合し、現在のGLBへKhronosを再実行する。
再生成した原本や別状態のPNGを古いreportで合格にしない。

無地previewは59°・yaw 0・既存RtT縦補正を合わせ、足元原点と3軸の投影点を0.01px以内で照合する。
既存sealed OCIO configをこの無地確認に限定して使い、陽性情報も記録する。一般Blender設定や
最終textureの色承認を置き換えない。TankはEmpty/Partial/Full、Mixerは停止姿勢と45°姿勢を出す。
静止2姿勢はゲーム内animation/pauseの証明ではない。catalogは同じ正方形の代表PNGを参照する。
UVは面ID付きの無地用planar mapping、材質はroughness=1 / metallic=0 / specular=0。
最終の面別atlas・描線・smoothingはゲーム内無地判定後に制作する。

2026-09-20の実測はBlender 5.1.1、4 GLBがscene errors/warnings=0、Khronos errors/warnings=0。
triangle数はTank body/water=`96/48`、Mixer body/rotor=`120/36`。
KhronosのUV未使用infoはplaceholder material exportにより外部albedo参照を含めないためで、UV0自体は後段で検査する。
Blender画像とtechnical passはArtPreview authority、アート承認、ゲーム内受入、releaseのいずれでもない。

### 既存Door / Wallのproduction経路

production Doorは`create-door-production-scene`で`Door_Closed` / `Door_Open` / `Door_Locked`を同じ原本へ生成し、`validate-door-glb`で単一node/mesh/primitive、状態別triangle数、固定枠signature、Open envelopeを検査します。`render-door-previews`はnetworkを切り、repositoryの`wall-calibration-v2.ocio`が陽性である場合だけEW/NSの固定canvasを出力します。`validate-door-textures`は512px Opaque albedo、256px RGBA preview、安全bbox、両軸hash差を検査します。

production Wall v2は上記legacy同期ではなく、art-approved final manifestに封印されたexact allowlistを隔離worktreeへ
provisionします。`--dest`は必ず対象worktreeのasset rootまで明示し、最初に同じ引数の`--dry-run`を確認します。

```bash
python3 scripts/sync_external_assets.py \
  --source "$ASSET_ROOT/staging/exports" \
  --dest "$VALIDATION_WORKTREE/assets" \
  --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set-final.json" \
  --selection core \
  --dry-run
```

この候補経路はmanifest外のfileと不採用normalをcoreへ混ぜず、primary / canonicalを同期先にしません。
pending candidateはruntime projection / allowlist同期のどちらも拒否します。release projectionとcanonical昇格は
別途generation-scoped promotion receiptを要求します。

外部workspaceを初期化・検査するコマンドは次の通りです。既存generic v1 templateは`--no-clobber`で保持され、
Wall v2の`generations/`、`authority/`、`quarantine/`だけが追加されます。

```bash
tools/blender_ai_workflow/bin/init-asset-workspace
tools/blender_ai_workflow/bin/verify-asset-workspace
```

final generationのcanonical昇格は制作・native検証とは別のM6承認操作です。通常はread-only planだけを作り、
plan / manifest / M5 evidence / release approvalを提示して承認を得るまでは`apply --confirm`を実行しません。
中断確認の`recover`とpreimage復帰の`rollback`も、`--apply`なしでは変更しません。

正本へ昇格する前に、`manifests/asset-manifest.template.json` をコピーして、
生成元、model/version、prompt/reference hash、ライセンス、Blender version、
出力 hash、report、reviewer を記録してください。API token やcookieは書きません。

## 7. Scene gate

現在の gate は、render-enabled な evaluated mesh に対して次を確認します。

- NaN/Infinity を含む transform、scale、vertex、UV
- zero/negative/unapplied scale
- triangle count と予算
- loose vertex、zero-area face、non-manifold、open boundary
- evaluated UV map
- polygonが参照する実material
- missing external image
- metric unit / scale
- Curve、Surface、Text、Metaball の未変換混入

warning は自動失敗にしませんが、昇格前に全件レビューします。
Khronos report は `numErrors == 0` を必須とし、warning も全件レビューします。

## 8. 環境スモーク

```bash
tools/blender_ai_workflow/bin/verify-mcp-addon
tools/blender_ai_workflow/bin/workflow-smoke
```

2026-07-31 の実測:

- Blender `5.1.1`
- scene: mesh `4`、triangles `432`
- scene validator: errors `0`、warnings `0`
- Khronos validator: errors `0`、warnings `0`
- TCP `scene.get_info`: 成功
- stdio MCP: 28 tools、read/save 成功
- Python/headless/direct export bypass: すべて拒否
- smoke GLB SHA-256:
  `a818b962edd7e4addf12c83e32f8571455f09291be1f4c631eaff0922e37e94d`

出力は `staging/{blend,exports,renders,reports}/ai_workflow_smoke*` にあります。

## 9. 再構築・更新

実装詳細と各コマンドは
[`tools/blender_ai_workflow/README.md`](../tools/blender_ai_workflow/README.md) を参照します。
vendor の固定値、依存lock、hardening patch は同ディレクトリの `vendor/` に置きます。

addon 更新時:

```bash
tools/blender_ai_workflow/bin/install-mcp-addon
tools/blender_ai_workflow/bin/verify-mcp-addon
```

更新後は vendor tests、workflow tests、MCP stdio smoke を再実行します。
upstream tag を動かす場合は、既存patchを無条件で当てず、差分を再監査してください。

## 10. 壁M0の色校正

壁の本番アート化では、Blender referenceとBevy actual-window captureを同じ5 patch contractで照合します。
契約の正本は`tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json`です。stone、rust、
dark-brown line、purpleの4色はunlit base-color経路としてD65 CIE Lab / CIEDE2000で比較し、
purple emissiveは相対輝度liftだけを別判定します。

Blender側は`render_color_calibration.py`で320×96、8-bit sRGB PNGとmetadataを
`staging/reports/`へ生成します。metadataには実際のOCIO config path / SHA-256、profile / runtime version、
明示configとactive configのcache ID一致、runtime validation、fallback状態、orthographic horizontal / vertical
span、color pipeline、入力色、PNG SHA-256、source fingerprintが必要です。
Bevy側artifactと揃った後、`verify_color_calibration.py`が中央16×16 px medianを再計算し、
4 patch平均Delta E 2000 `<= 2.0`かつ各patch`<= 3.0`、emissive sanityをfail-closedで判定します。
実行例と3つのgeometry / density / color fixtureの説明は
[`tools/blender_ai_workflow/README.md`](../tools/blender_ai_workflow/README.md)を参照してください。

現行Flatpakの同梱config 2.5 / runtime 2.4.2は`fallback=true`として正しく記録されます。壁の正式校正では
repo内の`tools/blender_ai_workflow/fixtures/wall-calibration-v2.ocio`（profile 2.1）を`OCIO`へ明示し、
`--require-ocio-positive`を付けます。このconfigはruntime validationとactive cache ID一致が実機でpassし、
exact sRGB transferを使います。dirty diagnostic artifactは正式証跡へ流用せず、clean subjectからBlender / Bevy
両artifactを採取してoffline gateを通すまで色承認とcanonical昇格を行いません。

## 11. 既知の制約

- Fedora Flatpak 同梱の OCIO config `2.5` は runtime OCIO `2.4.2` で読めずfallbackします。
  壁5 patch校正だけはsealed profile 2.1 configで回避しますが、一般Blender authoringをdefault configで
  色承認することはできません。
- `Material.use_nodes` は Blender 5.1.1 では動作しますが、6.0向けdeprecation warningが
  出ます。Blender upgradeとは別作業で移行します。
- 既存 `soul.glb` は Khronos validatorで既知errorがあり、P08でproduction / visual-test
  consumerも削除済みです。環境のgreen fixtureには使わず、現PCで新規canonical
  `.blend`を作る場合もローカルのcomparison referenceに限定します。importしたcopyを
  正本化しません。
- この構成は一般scene品質を検査します。Soulの製品表示はshared-pool billboardと
  P02 / P08 actual-window evidenceを正本とし、GLB clip / face atlasはrelease gateでは
  ありません。
