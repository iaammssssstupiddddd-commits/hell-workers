# rust-analyzer MCP の共有運用

## 目的

同じ Hell Workers workspace を複数の Codex / Claude / Gemini セッションで扱う場合、各 stdio MCP
client が独立した `rust-analyzer` を起動すると、Bevy を含む解析状態がセッション数分だけ常駐する。
`scripts/rust_analyzer_mcp_stdio_adapter.py` は既存の stdio 設定を保ったまま、workspace ごとに一つの
`rust-analyzer-mcp` backend を共有する。

## 共有される範囲

- canonical Git workspace root と Cargo / Rust toolchain 関連環境が同じ client は、一つの backend を共有する。
- 別 Git worktree、別 clone、または `CARGO_TARGET_DIR` / toolchain 設定が異なる client は意図的に別 backend
  になる。異なる source tree を誤って解析しないためである。
- backend は request を直列化する。複数 session の解析要求は安全に待機するが、同時に複数の
  `rust-analyzer` を作らない。
- shared backend では `rust_analyzer_set_workspace` を提供しない。workspace を切り替えると、接続中の
  他 client の解析対象も変わってしまうためである。

## ライフサイクル

- 最後の解析 request から既定で 5 分後に、重量の大きい backend と `rust-analyzer` child を終了する。
- client がすべて切断された場合、軽量な socket daemon も既定で 30 秒後に終了する。
- 次の MCP request は同じ workspace で backend を自動的に起動する。既存の MCP 設定を変更する必要はない。
- runtime socket は repository の `target/` ではなく、owner-only の XDG runtime directory（未設定時は
  user-specific temporary directory）に置く。

必要に応じて次の環境変数で調整する。

| 変数 | 既定値 | 用途 |
| --- | ---: | --- |
| `HELL_WORKERS_RA_MCP_BACKEND_IDLE_SECONDS` | `300` | Analyzer backend を解放するまでの無操作時間。 |
| `HELL_WORKERS_RA_MCP_DAEMON_IDLE_SECONDS` | `30` | 最後の client 切断後に socket daemon を終了するまでの時間。 |
| `HELL_WORKERS_RA_MCP_WORKSPACE` | なし | Git root を検出できない特殊な起動時に、共有対象の workspace root を明示する。 |
| `HELL_WORKERS_RA_MCP_RUNTIME_DIR` | なし | socket runtime directory を明示する。通常は test / 診断以外で不要。 |

## Zed と VS Code

Zed は stdio MCP backend とは別に独自の rust-analyzer LSP を持つため、Codex session と直接は共有しない。
project-local `.zed/settings.json` は cache priming、保持する syntax tree、全 target / 全 workspace の cargo
check を抑え、通常の保存時診断を維持したまま常駐コストを下げる。設定変更後は Zed が LSP を再起動するまで
待つか、window を再読み込みする。

VS Code の project setting も現行 rust-analyzer の `files.exclude` を用いて build artifact を解析対象から除く。

## 運用上の注意

- 既に起動済みの Codex session は古い adapter process を保持している。共有化を反映するには、その session を
  終了して新しく開始する。
- `target/` の下に作られた別 clone / Git repository から agent を起動すると、それは別 workspace として扱われ、
  追加の Analyzer が必要になる。通常の開発 session は repository root から開始する。
- backend が不要になったら agent session を閉じる。daemon は上記 timeout 内に自動解放するため、socket file を
  手作業で削除しない。
