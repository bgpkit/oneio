#!/usr/bin/env bash
set -euo pipefail

criterion_args=(-- --warm-up-time 1 --measurement-time 5 --sample-size 20)

run_case() {
  local name="$1"
  shift
  echo
  echo "== ${name} =="
  cargo bench "$@" "${criterion_args[@]}"
}

run_case "gzip miniz_oxide" --bench gzip_decompress --no-default-features --features gz-miniz
run_case "gzip zlib-rs" --bench gzip_decompress --no-default-features --features gz-zlib-rs
run_case "gzip zlib-ng" --bench gzip_decompress --no-default-features --features gz-zlib-ng
run_case "gzip cloudflare-zlib" --bench gzip_decompress --no-default-features --features gz-zlib-cloudflare
run_case "bzip2" --bench bzip2_decompress --no-default-features --features bz
