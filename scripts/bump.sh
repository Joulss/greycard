#!/bin/sh
# Set the workspace version and move every lockfile with it.
#
#     scripts/bump.sh X.Y.Z
#
# The fuzz targets are a workspace of their own, and their lockfile
# pins the greycard crates by version, so a bump of Cargo.toml alone
# leaves it stale and CI's `cargo check --locked` on it refuses.
# This edits the one line, then updates the greycard crates in both
# lockfiles and nothing else. Review and commit the four files
# together: Cargo.toml, Cargo.lock, fuzz/Cargo.lock, and the
# changelog's heading.

set -eu

cd "$(dirname "$0")/.."

version=${1:?usage: scripts/bump.sh X.Y.Z[-pre]}
# What release.yml takes from a tag: a digit first, then digits,
# letters, dots, plus, and minus (0.12.0, 0.12.0-rc.1). Cargo says
# whether the rest is a version; if either update fails, the three
# files go back as they were.
case "$version" in
[!0-9]* | *[!0-9A-Za-z.+-]*) echo "bump.sh: $version is not X.Y.Z[-pre]" >&2; exit 1 ;;
esac

for f in Cargo.toml Cargo.lock fuzz/Cargo.lock; do cp "$f" "$f.bak"; done
restore() {
    for f in Cargo.toml Cargo.lock fuzz/Cargo.lock; do mv "$f.bak" "$f"; done
    exit 1
}
sed -i.sed "s/^version = \"[^\"]*\"/version = \"$version\"/" Cargo.toml
rm Cargo.toml.sed
cargo update --workspace --offline --quiet || restore
cargo update --workspace --offline --quiet --manifest-path fuzz/Cargo.toml || restore
rm Cargo.toml.bak Cargo.lock.bak fuzz/Cargo.lock.bak

git --no-pager diff --stat -- Cargo.toml Cargo.lock fuzz/Cargo.lock
