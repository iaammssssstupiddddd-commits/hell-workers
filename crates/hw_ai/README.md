# hw_ai — 互換 Facade Crate

## 役割

`hw_familiar_ai` と `hw_soul_ai` を一つのエントリポイントとして再エクスポートする **互換 facade crate**。

この crate は直接ロジックを持たず、以下のシンボルを公開するのみです。

```rust
pub use hw_familiar_ai::familiar_ai;
pub use hw_soul_ai::soul_ai;
pub use hw_familiar_ai::FamiliarAiCorePlugin;
pub use hw_soul_ai::SoulAiCorePlugin;
```

AI の実装は以下の crate に分割されています:

| Crate | 内容 | ファイル数 |
|---|---|---|
| `hw_familiar_ai` | Familiar（監督）AI コアロジック | 63 |
| `hw_soul_ai` | Soul（労働者）AI コアロジック | 95 |

## 使い方

新規コードでは直接 `hw_familiar_ai` / `hw_soul_ai` を参照してください。
`hw_ai` 経由の import は互換性維持のためのみ残されています。

```toml
# 推奨 (新規コード)
hw_familiar_ai = { path = "../hw_familiar_ai" }
hw_soul_ai     = { path = "../hw_soul_ai" }

# 互換維持 (既存 import path を変えたくない場合)
hw_ai = { path = "../hw_ai" }
```

## 依存クレート

- `hw_familiar_ai`, `hw_soul_ai`

## 参照

- `crates/hw_familiar_ai/README.md` — Familiar AI 詳細
- `crates/hw_soul_ai/README.md` — Soul AI 詳細
- `docs/cargo_workspace.md` — workspace 境界設計
