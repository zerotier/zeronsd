FROM rust:latest as buildenv

ARG VERSION=main
ARG IS_TAG=0

RUN cargo install --git https://github.com/zerotier/zeronsd \
  $(if [ "${IS_TAG}" != "0" ]; then echo "--tag"; else echo "--branch"; fi) \
  "${VERSION}"

FROM debian:latest

RUN apt-get update -qq && apt-get install libssl1.1 -y && apt-get autoclean -y && apt-get clean -y
COPY --from=buildenv /usr/local/cargo/bin/zeronsd /usr/bin/zeronsd

ENTRYPOINT "/usr/bin/zeronsd"
CMD "help"
