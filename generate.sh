#!/bin/bash

set -x -euo pipefail

if [ "x$1" = "x" ]
then
  echo "Please read this script before executing it"
  exit 1
fi

PACKAGE=$1

if [ "x$2" = "x" ]
then
  HOST=apidocs.zerotier.com
else
  HOST=$2
fi


rm -rf ./${PACKAGE}
mkdir -p ./${PACKAGE}
docker pull openapitools/openapi-generator-cli:latest
docker run --rm -u $(id -u):$(id -g) -v ${PWD}/${PACKAGE}:/swagger openapitools/openapi-generator-cli generate \
  --package-name ${PACKAGE} \
  -i http://${HOST}/${PACKAGE}-v1/api-spec.json \
  -g rust \
  -o /swagger

grep -v default-features ${PACKAGE}/Cargo.toml > tmp && mv tmp ${PACKAGE}/Cargo.toml
