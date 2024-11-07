FROM --platform=$BUILDPLATFORM nixos/nix:latest AS builder

COPY . /tmp/build
WORKDIR /tmp/build

RUN nix \
    --extra-experimental-features "nix-command flakes" \
    --option filter-syscalls false \
    build

RUN mkdir /tmp/nix-store-closure
RUN cp -R $(nix-store -qR result/) /tmp/nix-store-closure

FROM scratch

WORKDIR /

# Copy /nix/store
COPY --from=builder /tmp/nix-store-closure /nix/store
COPY --from=builder /tmp/build/result/bin/zeronsd /bin/zeronsd
CMD ["/bin/zeronsd"]
