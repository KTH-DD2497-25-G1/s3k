#!/usr/bin/env bash
BASE=$(pwd)/tpm0

if ! [ -x "$(command -v swtpm)" ]; then
  echo "Error: swtpm is not installed." >&2
  exit 1
fi

if [ ! -d "${BASE}" ]; then
  mkdir -p "${BASE}"
fi

pkill -9 swtpm 2>/dev/null || true
mkdir -p "${BASE}"
rm -f "${BASE}/swtpm-sock"

swtpm socket \
  --tpm2 \
  --tpmstate dir="${BASE}" \
  --ctrl type=unixio,path="${BASE}/swtpm-sock" \
  --log file="${BASE}/swtpm.log",level=20 \
  --terminate \
  --daemon 

  sleep 0.2
