# Blender AI workflow tooling

Hell Workers の AI 支援 Blender 編集を、staging 限定・検証付きで運用するツール群です。
利用者向け手順は [`docs/blender-setup.md`](../../docs/blender-setup.md) を正とします。

## Commands

| Command | 用途 |
|---|---|
| `bin/init-asset-workspace` | 外部workspaceの階層と非秘密templateを安全に初期化 |
| `bin/blender` 相当の `blender-safe` | Flatpak filesystemを最小化し、embedded Python auto-runを無効化 |
| `bin/blender-ai` | 明示的な localhost MCP セッションを開始 |
| `bin/install-mcp-addon` | hardened addon zipを構築・導入し、既存userprefをローカル退避 |
| `bin/verify-mcp-addon` | 保存済み安全設定とwhitelistを再読込検証 |
| `bin/validate-blend` | evaluated sceneを検査して `staging/reports` へJSON出力 |
| `bin/export-staging-glb` | scene gate、GLB export、Khronos validatorを直列実行 |
| `bin/gltf-validate` | pinned Khronos validator wrapper |
| `bin/workflow-smoke` | deterministic `.blend` / PNG / GLB / reports を生成 |
| `scripts/render_color_calibration.py` | 壁M0の固定5 patchをBlenderで描画し、OCIO陽性証明付きmetadataを出力 |
| `scripts/verify_color_calibration.py` | Blender / Bevy PNGをCIEDE2000とemissive sanityでoffline照合 |
| `scripts/verify_wall_reference_locators.py` | 登録済みhistorical P02とcurrent fallback壁の用途・identity・artifact hashを分離検証 |

`validate-blend` と `export-staging-glb`:

```text
validate-blend <input.blend> <report-name.json> [max-triangles]
export-staging-glb <input.blend> <output-name.glb> [max-triangles]
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
  6 familyと16 maskのcanonical rotation。
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
  tools/blender_ai_workflow/bin/blender-safe \
  --background --factory-startup --python-exit-code 2 \
  --python tools/blender_ai_workflow/scripts/render_color_calibration.py -- \
  --contract tools/blender_ai_workflow/fixtures/wall-color-calibration-v1.json \
  --output "$ASSET_ROOT/staging/reports/wall-production-v1-color-reference.png" \
  --metadata "$ASSET_ROOT/staging/reports/wall-production-v1-color-reference.json" \
  --source-fingerprint '<approved-commit-and-tree-fingerprint>'
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

reference metadataは実際に使ったOCIO config path / SHA-256、runtime version、
`fallback=false`を必須とします。現行Fedora Flatpak Blender 5.1.1はconfig 2.5をruntime 2.4.2で
fallbackするため、geometry用diagnostic renderは可能でも色gateは必ずblockedになります。
fallbackを隠す、または4色の見た目だけでpass扱いにする運用は禁止です。
