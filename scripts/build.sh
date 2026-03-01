#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd -P)"

release=false
clean=false
keep_days=7
log_dir="logs"
target_dir="target"

usage() {
    cat <<'EOF'
Usage: ./scripts/build.sh [options]

Options:
  --release             Run release build
  --clean               Clean old log files before running
  --keep-days <days>    Keep log files newer than this many days (default: 7)
  --log-dir <dir>       Log directory (default: logs)
  --target-dir <dir>    Cargo target directory (default: target)
  -h, --help            Show this help
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            release=true
            shift
            ;;
        --clean)
            clean=true
            shift
            ;;
        --keep-days)
            keep_days="${2:-}"
            [[ -n "$keep_days" ]] || { echo "--keep-days requires a value" >&2; exit 2; }
            shift 2
            ;;
        --log-dir)
            log_dir="${2:-}"
            [[ -n "$log_dir" ]] || { echo "--log-dir requires a value" >&2; exit 2; }
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
mkdir -p "$log_dir"

if [[ "$clean" == "true" ]]; then
    find "$log_dir" -maxdepth 1 -type f \( -name "*.log" -o -name "*.txt" \) -mtime +"$keep_days" -print0 | xargs -0r rm -f --
fi

timestamp="$(date +%Y%m%d_%H%M%S)"
build_type="debug"
if [[ "$release" == "true" ]]; then
    build_type="release"
fi
error_log_file="$log_dir/build_error_${build_type}_${timestamp}.log"
combined_log_file="$log_dir/build_combined_${build_type}_${timestamp}.log"

build_args=(build --manifest-path "${PROJECT_ROOT}/Cargo.toml" --target-dir "$target_dir")
if [[ "$release" == "true" ]]; then
    build_args=(build --manifest-path "${PROJECT_ROOT}/Cargo.toml" --release --target-dir "$target_dir")
fi

echo "Running cargo build ($build_type)..."
echo "Error log: $error_log_file"
echo "Target dir: $target_dir"

if cargo "${build_args[@]}" >"$combined_log_file" 2>"$error_log_file"; then
    build_exit_code=0
else
    build_exit_code=$?
fi

"$SCRIPT_DIR/post-build-cleanup.sh" --max-size-gb 3 --target-dir "$target_dir" >/dev/null 2>&1 || true

if [[ $build_exit_code -ne 0 ]]; then
    echo
    echo "=== Build Errors ===" >&2
    if [[ -s "$error_log_file" ]]; then
        cat "$error_log_file" >&2
    fi

    if [[ -s "$combined_log_file" ]]; then
        {
            echo
            echo "=== Combined Output ==="
            cat "$combined_log_file"
        } >>"$error_log_file"
    fi

    echo
    echo "Full error log saved to: $error_log_file" >&2
    exit "$build_exit_code"
fi

if [[ -s "$combined_log_file" ]]; then
    cat "$combined_log_file"
fi

echo
echo "Build completed successfully!"
