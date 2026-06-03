#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="${ROOT_DIR}/docs/whitepaper/tool-mapping-protocol.md"
OUT_DIR="${ROOT_DIR}/docs/whitepaper/dist"
OUT="${OUT_DIR}/tool-mapping-protocol-whitepaper.pdf"

if ! command -v pandoc >/dev/null 2>&1; then
  echo "pandoc is required to render the white paper PDF." >&2
  exit 1
fi

if ! command -v typst >/dev/null 2>&1; then
  echo "typst is required as the pandoc PDF engine." >&2
  exit 1
fi

mkdir -p "${OUT_DIR}"

pandoc "${SRC}" \
  --from gfm+yaml_metadata_block \
  --standalone \
  --toc \
  --number-sections \
  --resource-path="${ROOT_DIR}/docs/whitepaper:${ROOT_DIR}" \
  --syntax-highlighting=tango \
  --pdf-engine=typst \
  -V papersize=us-letter \
  -o "${OUT}"

echo "Wrote ${OUT}"
