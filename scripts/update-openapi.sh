#!/usr/bin/env bash

set -eo pipefail

if [ "x$1" = "x" ]; then
  echo "Usage: $0 <central|service>"
  exit 1
fi

PACKAGE=$1
CLIENT_LIB_DIR="${PACKAGE}-api"

if [ ! -d ${CLIENT_LIB_DIR} ]; then
  echo "Missing ${CLIENT_LIB_DIR}"
  exit 1
fi

VERSION=${3:-"v1"}

URL_BASE="https://raw.githubusercontent.com/zerotier/docs/refs/heads/main/static/openapi/"

curl -sSL ${URL_BASE}/${PACKAGE}${VERSION}.json > ${CLIENT_LIB_DIR}/openapi.json
