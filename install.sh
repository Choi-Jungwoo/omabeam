#!/usr/bin/env bash
set -euo pipefail

if ! command -v omarchy >/dev/null 2>&1; then
  echo "OmaBeam setup requires Omarchy." >&2
  exit 1
fi

if ! command -v mise >/dev/null 2>&1; then
  echo "OmaBeam setup requires mise, which is included with Omarchy." >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  mise use --global --yes rust@latest
fi

mise use --global --yes --minimum-release-age 0s cargo:omabeam
mise exec -- omabeam setup
