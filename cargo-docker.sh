#!/bin/sh

if [ "${IS_LOCAL}" != 0 ]
then
  cargo install --release --path .
else
  cargo install --release --git https://github.com/zerotier/zeronsd \
    $(if [ "${IS_TAG}" != "0" ]; then echo "--tag"; else echo "--branch"; fi) \
    "${VERSION}"
fi
