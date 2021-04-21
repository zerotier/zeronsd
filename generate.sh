#!/bin/bash

if [ "x$1" = "x" ]
then
  echo "Please read this script before executing it"
  exit 1
fi

PACKAGE=$1

rm -rf ./${PACKAGE}
mkdir -p ./${PACKAGE}
docker pull openapitools/openapi-generator-cli:latest
docker run --rm -u $(id -u):$(id -g) -v ${PWD}/schemas/${PACKAGE}.json:/api-spec.json -v ${PWD}/${PACKAGE}:/swagger openapitools/openapi-generator-cli generate \
  --package-name ${PACKAGE} \
  -i /api-spec.json \
  -g rust \
  -o /swagger

grep -v default-features ${PACKAGE}/Cargo.toml > tmp && mv tmp ${PACKAGE}/Cargo.toml
