#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
  set -- test-fast
fi

if [[ "${XAC_FAST_LINKER:-1}" != "0" ]] && command -v mold >/dev/null 2>&1; then
  case " ${RUSTFLAGS:-} " in
    *" -fuse-ld=mold "*) ;;
    *) export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-fuse-ld=mold" ;;
  esac
fi

if [[ "${1:-}" == "test-fast" ]]; then
  shift
  if cargo nextest --version >/dev/null 2>&1; then
    exec cargo nextest run --workspace "$@"
  fi
  exec cargo test --workspace "$@"
fi

exec cargo "$@"
