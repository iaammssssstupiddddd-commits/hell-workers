# Hell Workers asset workspace

このrootはリポジトリ外の制作・受入領域です。

- `source/`: 承認済み原本
- `staging/`: AI生成物、変換コピー、作業中 `.blend`、検証前export、report
- `exports/`: 承認済みのゲーム投入候補
- `manifests/`: provenance、hash、review記録
- `licenses/`: ライセンス本文または参照記録

自動化は `source/` と `exports/` へ直接書きません。`staging/reports` のscene検査、
Khronos検査、render目視、license/provenanceを確認し、人が承認した場合だけ昇格します。

秘密値、API token、cookie、Blender `userpref.blend` をこの同期rootへ置かないでください。
