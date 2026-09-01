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
