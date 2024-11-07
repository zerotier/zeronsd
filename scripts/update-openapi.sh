#!/usr/bin/env bash

set -euxo pipefail

if [ "x$1" = "x" ]; then
  echo "Usage: $0 <central|service> [version='v1']"
  exit 1
fi

PACKAGE=$1
VERSION=${2:-"v1"}

OPENAPI_FILE="${PACKAGE}${VERSION}.json"
CLIENT_LIB_DIR="zerotier-api"

if [ ! -d ${CLIENT_LIB_DIR} ]; then
  echo "Missing ${CLIENT_LIB_DIR}"
  exit 1
fi


URL_BASE="https://raw.githubusercontent.com/zerotier/docs/refs/heads/main/static/openapi/"

curl -sSL ${URL_BASE}/${OPENAPI_FILE} > ${CLIENT_LIB_DIR}/specs/${OPENAPI_FILE}
