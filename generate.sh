#!/bin/bash

set -x -euo pipefail

if [ "x$1" = "x" ]
then
  echo "Please read this script before executing it"
  exit 1
fi

PACKAGE=$1
PREFIX=$2

HOST=${HOST:-docs.zerotier.com}

rm -rf ./${PREFIX}
mkdir -p ./${PREFIX}
docker pull openapitools/openapi-generator-cli:latest
docker run --rm -u $(id -u):$(id -g) -v ${PWD}/${PREFIX}:/swagger openapitools/openapi-generator-cli generate \
  --package-name ${PREFIX} \
  -i http://${HOST}/openapi/${PACKAGE}v1.json \
  -g rust \
  -o /swagger

grep -v default-features ${PREFIX}/Cargo.toml > tmp && mv tmp ${PREFIX}/Cargo.toml
