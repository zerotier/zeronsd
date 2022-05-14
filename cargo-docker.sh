#!/bin/sh

export PATH="${HOME}/.cargo/bin:${PATH}"

if [ "${IS_LOCAL}" != 0 ]
then
  cargo install --path .
else
  cargo install --git https://github.com/zerotier/zeronsd \
    $(if [ "${IS_TAG}" != "0" ]; then echo "--tag"; else echo "--branch"; fi) \
    "${VERSION}"
fi
