#!/bin/sh
set -eu
cd "$(dirname "$0")" || exit

VERSION="$(git describe --tags --abbrev=0)"

export RUSTFLAGS="--remap-path-prefix /home/${USER}=/build"
cargo build --release --target x86_64-unknown-linux-musl
cp target/x86_64-unknown-linux-musl/release/rres "./rres-${VERSION}-x86_64-unknown-linux-musl"
strip "rres-${VERSION}-x86_64-unknown-linux-musl"
if [ -n "$(strings "rres-${VERSION}-x86_64-unknown-linux-musl" | rg "${USER}")" ]; then
    echo "release contains '${USER}'" && exit 1
fi
