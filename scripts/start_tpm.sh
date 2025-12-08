#!/usr/bin/env bash
BASE=/home/deenka/repos/system-sec-course/rs-s3k/tpm0


pkill -9 swtpm 2>/dev/null || true
mkdir -p "${BASE}"
rm -f "${BASE}/swtpm-sock"

swtpm socket \
  --tpm2 \
  --tpmstate dir="${BASE}" \
  --ctrl type=unixio,path="${BASE}/swtpm-sock" \
  --log file="${BASE}/swtpm.log",level=20 \
  --daemon 

  sleep 0.2
