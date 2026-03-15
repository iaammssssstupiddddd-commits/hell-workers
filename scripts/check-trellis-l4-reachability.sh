#!/usr/bin/env bash
# trellis-l4 への疎通を定期的に確認する。生成中に VM が固まっていないか監視する用。
# 使い方: ./scripts/check-trellis-l4-reachability.sh
# 停止: Ctrl+C

set -e
HOST="${1:-gcp-trellis-l4}"
INTERVAL="${2:-15}"

echo "Checking reachability to $HOST every ${INTERVAL}s (Ctrl+C to stop)"
echo "---"

while true; do
  if ssh -o ConnectTimeout=10 -o BatchMode=yes "$HOST" "echo ok" 2>/dev/null; then
    echo "$(date -Iseconds) OK"
  else
    echo "$(date -Iseconds) FAIL (timeout or connection refused)"
  fi
  sleep "$INTERVAL"
done
