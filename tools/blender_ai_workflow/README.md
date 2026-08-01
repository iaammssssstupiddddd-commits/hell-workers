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
