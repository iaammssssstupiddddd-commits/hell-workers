# Blender AI workflow tooling

Hell Workers の AI 支援 Blender 編集を、staging 限定・検証付きで運用するツール群です。
利用者向け手順は [`docs/blender-setup.md`](../../docs/blender-setup.md) を正とします。

## Commands

| Command | 用途 |
|---|---|
| `bin/init-asset-workspace` | 外部workspaceの階層と非秘密templateを安全に初期化 |
| `bin/verify-asset-workspace` | generic v1とWall v2 generation workspaceをread-only検査 |
| `bin/blender` 相当の `blender-safe` | Flatpak filesystemを最小化し、embedded Python auto-runを無効化 |
| `bin/blender-ai` | 明示的な localhost MCP セッションを開始 |
| `bin/install-mcp-addon` | hardened addon zipを構築・導入し、既存userprefをローカル退避 |
| `bin/verify-mcp-addon` | 保存済み安全設定とwhitelistを再読込検証 |
| `bin/validate-blend` | evaluated sceneを検査して `staging/reports` へJSON出力 |
| `bin/export-staging-glb` | scene gate、GLB export、Khronos validatorを直列実行 |
| `bin/gltf-validate` | pinned Khronos validator wrapper |
| `bin/validate-wall-glb` | production Wall GLBの構造・bounds・port断面をbytesから再検証 |
| `bin/create-wall-production-scene` | shared textureから6 familyの決定的Wall `.blend`を生成 |
| `bin/render-wall-reference-board` | 6 familyを59.036° Orthographic / OCIO陽性条件で描画 |
| `scripts/validate_asset_set_manifest.py` | Wall asset-set manifest v2と全参照artifactをexact検証 |
| `scripts/validate_wall_textures.py` | shared textureとoptional +Y normal candidateをpixel検証 |
| `scripts/verify_wall_rebuild.py` | 6 GLBとpost-export構造値の独立rebuild一致を検証 |
| `scripts/seal_wall_candidate.py` | clean Git主体と外部stagingのclosed setからcandidate manifest v2を封印 |
| `scripts/promote_asset_set.py` | Wall final generationのplan / apply / recover / rollback transaction |
| `bin/workflow-smoke` | deterministic `.blend` / PNG / GLB / reports を生成 |
| `scripts/render_color_calibration.py` | 壁M0の固定5 patchをBlenderで描画し、OCIO陽性証明付きmetadataを出力 |
| `scripts/verify_color_calibration.py` | Blender / Bevy PNGをCIEDE2000とemissive sanityでoffline照合 |
| `scripts/verify_wall_reference_locators.py` | 登録済みhistorical P02とcurrent fallback壁の用途・identity・artifact hashを分離検証 |

`validate-blend` と `export-staging-glb`:

```text
validate-blend <input.blend> <report-name.json> [max-triangles] [--collection <exact-name>]
export-staging-glb <input.blend> <output-name.glb> [max-triangles] [--collection <exact-name>] [--geometry-scale <positive>] [--materials-mode <export|placeholder|none>]
validate-wall-glb <input.glb> <family> <report-name.json>
```

`--collection`はM1 wall asset専用のopt-in selectorです。exact collectionがunknown / empty、または
render-enabled meshが1個でなければexport前に失敗します。指定しない既存scene全体の検査・export contractは
変更しません。collection export後はKhronos validatorに加えて`validate-wall-glb`を実行し、GLBのJSON / BINを
直接decodeして1 node / 1 mesh / 1 primitive、identity node、UV0、tangent有無、embedded image 0、350 triangle
cap、raw Y `-16..+16 wu`、9.6 wu port profile、12.8 wu corridor unionを検証します。

production Wall sceneは各familyを1 tile単位、9.6 / 32 = 0.30 tile厚、上下`-0.5..+0.5 tile`でauthoringし、
上下2つのside material bandを持つ216〜240 trianglesへ決定的に分割します。正式exportだけはscene gate後のin-memory mesh copyへ
`--geometry-scale 32 --materials-mode placeholder`を適用し、object transformをidentityのままraw wuへbakeします。
Blender 5.1.1のglTF operatorにglobal scale propertyがないため、このopt-in頂点bakeを使います。既存exportの
既定はscale 1 / material exportのままです。

```bash
tools/blender_ai_workflow/bin/create-wall-production-scene
tools/blender_ai_workflow/bin/render-wall-reference-board
tools/blender_ai_workflow/bin/export-staging-glb \
  "$ASSET_ROOT/staging/blend/wall-production-v1.blend" \
  models/buildings/wall/wall_isolated.glb \
  350 \
  --collection Wall_Isolated \
  --geometry-scale 32 \
  --materials-mode placeholder
```

Wall asset-set manifest v2は、既存の単体asset manifest v1とは別schemaです。candidate検証は
`normal_decision=pending`（core 8 file＋optional normal 1 file）とpending runtime subject / candidate art reviewを許し、
final検証はnormalをadopted（core 9）またはrejected（core 8）へ確定し、runtime subjectとhash付きのart approval
artifactを必須にします。6 familyのGLB、scene / export / Khronos / post-export report、set report、1024角texture、
source `.blend`、geometry contract、tool commit / tree / version、provenance、licenseをclosed field setとactual bytesの
role / byte length / SHA-256で検査します。不変identity fieldは`asset_set_generation`であり、runtimeの
`activation_revision`とは別物です。

```bash
python3 tools/blender_ai_workflow/scripts/validate_asset_set_manifest.py \
  --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" \
  --mode candidate \
  --blend-root "$ASSET_ROOT/staging/blend" \
  --exports-root "$ASSET_ROOT/staging/exports" \
  --reports-root "$ASSET_ROOT/staging/reports" \
  --licenses-root "$ASSET_ROOT/licenses" \
  --repo "$PWD"
```

validatorはmanifestの`tool_commit` / `tool_tree`を指定repoのHEADと照合します。templateは必要fieldを示すための
未封印雛形であり、空hashのまま検証を通るサンプルではありません。

候補を隔離worktreeへprovisionする時は、repository側の同期scriptをmanifest modeで使います。
`core`はexact 8 file、`optional:normal`はpending候補のnormal 1 fileだけを扱います。manifest外file、symlink、
`--delete-missing`併用、promotion receipt未対応のfinal manifestは拒否されます。

```bash
python3 scripts/sync_external_assets.py \
  --source "$ASSET_ROOT/staging/exports" \
  --dest "$VALIDATION_WORKTREE/assets" \
  --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" \
  --selection core \
  --dry-run
```

final asset-setのcanonical promotionは既定でread-onlyな`plan`から開始します。planはcurrent active pointerの
preimage、manifest / payload hash、単調generation、asset root、root外snapshotをcanonical JSONへ封印します。
`apply`は別途承認されたM5 evidence、release approval、固有receipt ID、UTC時刻、`--confirm`がすべて必要です。

```bash
python3 tools/blender_ai_workflow/scripts/promote_asset_set.py plan \
  --manifest "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json" \
  --asset-root "$ASSET_ROOT" \
  --snapshot "$SNAPSHOT_ROOT/wall-production-v1-preimage.json" \
  --output "$ASSET_ROOT/staging/reports/wall-production-v1.promotion-plan.json"
```

payload / evidence / immutable receiptは同一temporary generation内でhash検証・file / directory fsyncした後、
generation directoryを一度だけrenameします。最後に唯一のmutable authorityであるactive pointerをatomic replaceし、
親directoryをfsyncします。`recover` / `rollback`も既定はread-onlyで、変更には各々`--apply`が必要です。
rollbackはgenerationを削除せず、root外snapshotに封印したprevious pointerだけを復元します。

texture candidateは1024×1024 opaque RGBを必須とし、emissiveのactive pixelが紫UV領域外へ漏れないこと、
optional normalがlinear sampling / OpenGL `+Y`で、vector length・positive Z条件を満たすことを検査します。

```bash
python3 tools/blender_ai_workflow/scripts/validate_wall_textures.py \
  --texture-root "$ASSET_ROOT/staging/exports/textures/buildings/wall" \
  --report "$ASSET_ROOT/staging/reports/wall-production-v1.textures.json" \
  --normal-decision pending \
  --normal-sampling linear \
  --normal-convention +Y
```

Candidate sealerはcleanなrepository `HEAD` / tree、6 mesh、texture report、reference board、独立rebuild、
prompt provenance、license evidenceを1つのmanifestへhash結合します。検証が完了するまで既存manifestを置換しません。

```bash
python3 tools/blender_ai_workflow/scripts/seal_wall_candidate.py \
  --asset-root "$ASSET_ROOT" \
  --repo . \
  --generation 1 \
  --provenance "$ASSET_ROOT/source/generated/wall-production-v1/provenance.json" \
  --license-metadata "$ASSET_ROOT/source/generated/wall-production-v1/license.json" \
  --output "$ASSET_ROOT/staging/reports/wall-production-v1.asset-set.json"
```

## Environment

| Variable | Default |
|---|---|
| `HELL_WORKERS_ASSET_ROOT` | `$HOME/Sync/hell-workers-assets` |
| `BLENDER_MCP_ROOT` | `$HOME/tools/blender-mcp-server` |
| `GLTF_VALIDATOR_ROOT` | `$HOME/tools/gltf-validator` |
| `BLENDER_SAFE_NO_NETWORK` | `0`; batch wrappers force `1` |

`bin/blender-mcp-server` always forces
`BLENDER_MCP_ALLOW_PYTHON_EXEC=0` and `BLENDER_MCP_ALLOW_HEADLESS=0`。

## Pinned vendor

- repository: `https://github.com/djeada/blender-mcp-server`
- tag: `v0.1.3`
- commit: `7eed33edf4aca2ab0ca84a6da27321f89f68b504`
- local clone: `$HOME/tools/blender-mcp-server`
- patch: `vendor/blender-mcp-v0.1.3-hardening.patch`
- Python resolution: `vendor/blender-mcp-python.lock`

再構築時は exact commit を clone し、patch を `git apply` してから venv を作ります。

```bash
PROJECT_ROOT="${PROJECT_ROOT:-$HOME/projects/hell-workers}"
MCP_ROOT="${BLENDER_MCP_ROOT:-$HOME/tools/blender-mcp-server}"

git clone https://github.com/djeada/blender-mcp-server \
  "$MCP_ROOT"
git -C "$MCP_ROOT" \
  checkout 7eed33edf4aca2ab0ca84a6da27321f89f68b504
git -C "$MCP_ROOT" \
  apply "$PROJECT_ROOT/tools/blender_ai_workflow/vendor/blender-mcp-v0.1.3-hardening.patch"
python3 -m venv "$MCP_ROOT/.venv"
"$MCP_ROOT/.venv/bin/pip" install \
  -r "$PROJECT_ROOT/tools/blender_ai_workflow/vendor/blender-mcp-python.lock"
"$MCP_ROOT/.venv/bin/pip" install \
  --no-deps -e "$MCP_ROOT"
```

Khronos validator:

```bash
cd "$HOME/tools/gltf-validator"
npm ci --ignore-scripts
```

`package-lock.json` が `gltf-validator 2.0.0-dev.3.10` を固定します。

## Verification

```bash
cd "$HOME/tools/blender-mcp-server"
.venv/bin/pytest -q
.venv/bin/ruff check .
.venv/bin/mypy src

cd "$HOME/projects/hell-workers"
python3 tools/blender_ai_workflow/tests/test_script_contracts.py
"$HOME/tools/blender-mcp-server/.venv/bin/ruff" check \
  tools/blender_ai_workflow/scripts \
  tools/blender_ai_workflow/tests
tools/blender_ai_workflow/bin/verify-mcp-addon
tools/blender_ai_workflow/bin/workflow-smoke
```

MCPの完全な疎通検査では `scripts/bridge_smoke_server.py` を起動中に、
vendor venv のPythonで `scripts/mcp_smoke_client.py` を実行します。

## Production wall M0 contracts

壁の本番アート化M0は、次のmachine-readable fixtureを正とします。

- `fixtures/wall-production-v1.geometry.json`: 32 wu cell、9.6 wu公称厚、12.8 wu装飾外形、
  6 familyと16 maskのcanonical rotation。`bounds.contract_kind=maximum_cell_envelope`は全family共通の
  cell内許容外形で、raw meshのYだけを`-16..+16 wu`へ固定し、配置中心Y=16 wuで接地します。
- `fixtures/wall-production-v1.orientation.svg`: geometry JSONのSHA-256へ結合した上面／側面図。
  N=-Z、E=+X、+Y正回転のN→W、6 canonical family、中心pivot、identity node、world Y=0..32を示します。
- `fixtures/wall-density-v1.json`: N=96 / 4N=384、20列・5 cell strideのexact配置、
  Door blueprint connector数、camera scale、seed、画面・renderer・計測時間、
  completed / provisionalのdraw predicate。profiling runtimeは同じbytesのSHA-256をpinし、
  target / connector全行とphase別layout checksumをsidecarへ出す。
- `fixtures/wall-color-calibration-v1.json`: 4 base patchとemissive sanity patch、
  PNG / ROI / color pipeline、CIEDE2000閾値。
- `fixtures/wall-reference-locators-v1.json`: 登録済みhistorical P02 presentation artifactと
  current fallback wall actual-window artifactのauthority、用途外範囲、identity、SHA-256。

historical P02は過去の表示・性能契約でありcurrent wall pixelの正本ではありません。current fallback wallは
現行画像の比較参照でありhistorical P02性能やproduction art承認には使いません。登録index、checksum ledger、
P02 attempt / RenderDoc、current manifest / observation / PNGを一括検証するには次を実行します。

```bash
python3 tools/blender_ai_workflow/scripts/verify_wall_reference_locators.py
```

Blender referenceはcanonical assetではなく外部`staging/reports/`へだけ出力します。
既存artifactを上書きしないため、正式採取では承認済みsource fingerprintを名前と引数へ含めます。

```bash
ASSET_ROOT="${HELL_WORKERS_ASSET_ROOT:-$HOME/Sync/hell-workers-assets}"

BLENDER_SAFE_NO_NETWORK=1 \
  OCIO="$PWD/tools/blender_ai_workflow/fixtures/wall-calibration-v2.ocio" \
  tools/blender_ai_workflow/bin/blender-safe \
  --background --factory-startup --python-exit-code 2 \
  --python tools/blender_ai_workflow/scripts/render_color_calibration.py -- \
  --contract tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json \
  --output "$ASSET_ROOT/staging/reports/wall-production-v1-color-reference.png" \
  --metadata "$ASSET_ROOT/staging/reports/wall-production-v1-color-reference.json" \
  --source-fingerprint '<approved-commit-and-tree-fingerprint>' \
  --require-ocio-positive
```

Bevy actual-window phaseが同じcontractのcandidate PNG / metadataを生成した後、offline gateを実行します。

```bash
python3 tools/blender_ai_workflow/scripts/verify_color_calibration.py \
  --contract tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json \
  --reference "$ASSET_ROOT/staging/reports/wall-production-v1-color-reference.png" \
  --reference-metadata "$ASSET_ROOT/staging/reports/wall-production-v1-color-reference.json" \
  --candidate "$ASSET_ROOT/staging/reports/wall-production-v1-color-candidate.png" \
  --candidate-metadata "$ASSET_ROOT/staging/reports/wall-production-v1-color-candidate.json" \
  --output "$ASSET_ROOT/staging/reports/wall-production-v1-color-verification.json"
```

reference metadataは実際に使ったOCIO config path / SHA-256、profile / runtime version、明示configと
active configのcache ID一致、runtime validation pass、`fallback=false`を必須とします。Fedora Flatpak
Blender 5.1.1の同梱config 2.5はruntime 2.4.2で読めないため、正式採取では壁校正専用の
`fixtures/wall-calibration-v2.ocio`（OCIO profile 2.1、exact sRGB transfer）を`OCIO`へ明示し、
`--require-ocio-positive`を付けます。default config、fallbackを隠す、または4色の見た目だけでpass扱いにする
運用は禁止です。この専用configは壁5 patch校正だけの契約で、一般Blender authoringの色設定を置き換えません。
