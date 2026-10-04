# 外付け統括構想の移管先

2026-10-03、外付け統括の構想を完全別プロジェクト **Orca Conductor** へ移管した。
配置先は `/home/satotakumi/projects/orca-conductor`。
移管先の `README.md` を入口とする。repository内リンクへローカル絶対pathを埋め込まない。

設計の正本、開発規則、今後の実装計画は移管先で管理する。hell-workersのsubmodule、worktree、
Cargo workspace member、通常起動の依存にはしない。本書は移管案内だけで、外付け機能の仕様書ではない。
移管時点は構想保存のみであり、workflowやpluginは未実装・未導入。

元提案 `docs/proposals/orca-optional-coordination-proposal-2026-10-03.md` の本文は、移管先の
`docs/history/original-concept-2026-10-03.txt` へbyte同一で保存してから本repositoryから除去した。
SHA256: `bfcc822b598c72e7eba5a8c3356e04ec215001b391d4fba6847a453314c48cf7`。
旧実装・未commit成果・会話・Run・unknown receiptの既存保全は変更せず、旧指示を再実行しない。

hell-workersのゲーム機能、Help、品質gate、通常Orcaの設定への変更はない。
構想分離を旧案件の完了や製品工程の再開と読み替えない。
