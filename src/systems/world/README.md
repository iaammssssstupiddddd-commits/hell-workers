# world — ワールドゾーン・Site/Yard 管理

## 役割

ゲームワールドのゾーン（Site・Yard）管理を行うシステム群。
地形生成・座標変換・経路探索は `hw_world` クレートに実装されており、このディレクトリは**ルートクレート固有のゾーン抽象**を担う。

## ファイル一覧

| ファイル | 内容 |
|---|---|
| `mod.rs` | `Site`, `Yard`, `PairedSite`, `PairedYard` の公開 |
| `zones.rs` | `Site`・`Yard` コンポーネントと `PairedSite`/`PairedYard` Relationship 定義 |

## 主要型

```rust
Site       // 採取・採掘等の作業サイトエンティティ
Yard       // 素材保管・中間処理ヤードエンティティ
PairedSite // Yard → Site の ECS Relationship
PairedYard // Site → Yard の ECS Relationship
```

Site と Yard は ECS Relationship でペアリングされ、互いを参照できる。

## 関連ドキュメント

地形・経路探索については `hw_world/README.md` を参照。
