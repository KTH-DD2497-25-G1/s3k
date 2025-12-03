#!/usr/bin/env bash
mkdir tpm0
swtpm socket \
  --tpm2 \
  --tpmstate dir=/home/deenka/repos/system-sec-course/rs-s3k/tpm0 \
  --ctrl type=unixio,path=/home/deenka/repos/system-sec-course/rs-s3k/tpm0/swtpm-sock \
  --log file=tpm0/swtpm.log,level=20
