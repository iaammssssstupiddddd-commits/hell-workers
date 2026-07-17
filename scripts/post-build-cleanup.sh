#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd -P)"

max_size_gb=3
target_dir="target"

usage() {
    cat <<'EOF'
Usage: ./scripts/post-build-cleanup.sh [options]

Options:
  --max-size-gb <n>     Warn when target dir size exceeds this value (default: 3)
  --target-dir <dir>    Cargo target directory (default: target)
  -h, --help            Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --max-size-gb)
            max_size_gb="${2:-}"
            [[ -n "$max_size_gb" ]] || { echo "--max-size-gb requires a value" >&2; exit 2; }
            shift 2
            ;;
        --target-dir)
            target_dir="${2:-}"
            [[ -n "$target_dir" ]] || { echo "--target-dir requires a value" >&2; exit 2; }
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

cd "$PROJECT_ROOT"

get_directory_size_bytes() {
    local path="$1"
    if [[ ! -d "$path" ]]; then
        echo 0
        return
    fi

    du -sb "$path" 2>/dev/null | awk '{print $1}'
}

to_gb_text() {
    local bytes="$1"
    awk -v b="$bytes" 'BEGIN {printf "%.2f", b/1024/1024/1024}'
}

max_size_bytes=$((max_size_gb * 1024 * 1024 * 1024))

# Remove cross compile artifacts when they are large.
x86_path="${target_dir}/x86_64-pc-windows-msvc"
if [[ -d "$x86_path" ]]; then
    x86_size="$(get_directory_size_bytes "$x86_path")"
    if (( x86_size > 100 * 1024 * 1024 )); then
        echo "Removing unnecessary x86_64-pc-windows-msvc directory ($(to_gb_text "$x86_size") GB)..."
        rm -rf "$x86_path"
        echo "Removed"
    fi
fi

current_size="$(get_directory_size_bytes "$target_dir")"
if (( current_size > max_size_bytes )); then
    echo "Target directory size ($(to_gb_text "$current_size") GB) exceeds limit (${max_size_gb} GB)"
    echo "Clean up old build artifacts if needed (e.g. debug/build or old incremental caches)."
fi
