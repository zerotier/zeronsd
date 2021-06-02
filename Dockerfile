FROM rust:latest as buildenv

ARG IS_LOCAL=0
ARG VERSION=main
ARG IS_TAG=0

COPY . /code
WORKDIR /code
RUN sh cargo-docker.sh

FROM debian:latest

RUN apt-get update -qq && apt-get install libssl1.1 ca-certificates -y && apt-get autoclean -y && apt-get clean -y
COPY --from=buildenv /usr/local/cargo/bin/zeronsd /usr/bin/zeronsd

ENTRYPOINT ["/usr/bin/zeronsd"]
