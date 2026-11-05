#!/usr/bin/env bash
# Seeds fuzz/corpus/<target>/ with the sample images checked into the repo and,
# when available, PngSuite (http://www.schaik.com/pngsuite/).
#
# Usage: fuzz/scripts/seed-corpus.sh [--download-pngsuite]
#
# PngSuite is looked up in $PNGSUITE_DIR (default: <repo>/PngSuite, which is
# git-ignored). With --download-pngsuite it is fetched there if missing.
set -euo pipefail

fuzz_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_dir="$(cd "$fuzz_dir/.." && pwd)"
pngsuite_dir="${PNGSUITE_DIR:-$repo_dir/PngSuite}"
pngsuite_url="http://www.schaik.com/pngsuite/PngSuite-2017jul19.tgz"

if [[ "${1:-}" == "--download-pngsuite" && ! -d "$pngsuite_dir" ]]; then
  mkdir -p "$pngsuite_dir"
  if ! curl -fsSL --retry 3 "$pngsuite_url" | tar -xz -C "$pngsuite_dir"; then
    echo "warning: could not download PngSuite; continuing with in-repo samples" >&2
    rm -rf "$pngsuite_dir"
  fi
fi

png_seeds=("$repo_dir"/crates/png-glitch/etc/*.png)
if [[ -d "$pngsuite_dir" ]]; then
  while IFS= read -r -d '' f; do png_seeds+=("$f"); done \
    < <(find "$pngsuite_dir" -name '*.png' -print0)
fi
webp_seeds=("$repo_dir"/crates/webp-glitch/etc/*.webp)

seed() {
  local target="$1"; shift
  mkdir -p "$fuzz_dir/corpus/$target"
  cp -f "$@" "$fuzz_dir/corpus/$target/"
  echo "corpus/$target: $(find "$fuzz_dir/corpus/$target" -type f | wc -l) files"
}

seed png_parse "${png_seeds[@]}"
seed png_glitch_ops "${png_seeds[@]}"
seed glitch_context_pipeline "${png_seeds[@]}" "${webp_seeds[@]}"
seed webp_riff "${webp_seeds[@]}"
