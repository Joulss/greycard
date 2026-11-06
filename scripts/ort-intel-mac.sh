#!/bin/sh
# Put greycard's own ONNX Runtime for an Intel Mac beside the binaries
# of each profile named, where scripts/ort-dylib.sh puts the library
# on the other platforms.
#
#     scripts/ort-intel-mac.sh [profile ...]  (debug and release if none)
#
# pyke makes no x86-64 macOS build to link one from, and Microsoft's
# last Intel Mac release is 1.23.2, older than the API ort binds, so
# the library is built from Microsoft's v1.28.0 source by
# .github/workflows/onnxruntime-intel-mac.yml: a shared library with
# Core ML and without WebGPU, so no Dawn beside it. That build is run
# once, by hand, and its archive attached to this repository's release
# tagged onnxruntime-1.28.0-x86_64-macos; this fetches it from there,
# checks it against the SHA-256 below, and copies the library in as
# target/<profile>/libonnxruntime.dylib, which package.sh ships.
#
# The download goes to target/ort-intel-mac/, never to a cache outside
# the tree, and a second run takes it from there. CARGO_TARGET_DIR is
# honored, as cargo honors it.

set -eu

cd "$(dirname "$0")/.."

version=1.28.0
name="onnxruntime-$version-x86_64-macos-coreml"
url="https://github.com/jessolmstead/greycard/releases/download/onnxruntime-$version-x86_64-macos/$name.tar.gz"
# What onnxruntime-intel-mac.yml printed for the archive attached to
# the release. Empty until that build has been run and attached.
sha256=a6862c380be3a4d5812c97596bd59a6570baaee9d8d6ebfa3defb050f83057b9

host=$(rustc -vV | sed -n 's/^host: //p')
if [ "$host" != x86_64-apple-darwin ]; then
    echo "ort-intel-mac.sh: this library is for x86_64-apple-darwin, and rustc's host is $host" >&2
    exit 1
fi
if [ -z "$sha256" ]; then
    echo "ort-intel-mac.sh: no SHA-256 for $name.tar.gz yet: run" \
        "onnxruntime-intel-mac.yml, attach its archive to the release" \
        "onnxruntime-$version-x86_64-macos, and put the hash it printed here" >&2
    exit 1
fi

hash_of() {
    shasum -a 256 "$1" | awk '{ print $1 }'
}

target=${CARGO_TARGET_DIR:-target}
work="$target/ort-intel-mac"
archive="$work/$name.tar.gz"
mkdir -p "$work"
if [ ! -f "$archive" ] || [ "$(hash_of "$archive")" != "$sha256" ]; then
    rm -f "$archive" "$archive.part"
    curl -fL --retry 3 --max-time 600 -o "$archive.part" "$url"
    got=$(hash_of "$archive.part")
    if [ "$got" != "$sha256" ]; then
        rm -f "$archive.part"
        echo "ort-intel-mac.sh: $url has SHA-256 $got, not $sha256" >&2
        exit 1
    fi
    mv "$archive.part" "$archive"
fi

rm -rf "${work:?}/$name"
tar -xzf "$archive" -C "$work"
lib="$work/$name/libonnxruntime.$version.dylib"
if [ ! -f "$lib" ]; then
    echo "ort-intel-mac.sh: $archive has no $name/libonnxruntime.$version.dylib" >&2
    exit 1
fi

[ $# -gt 0 ] || set -- debug release
for profile; do
    mkdir -p "$target/$profile"
    cp "$lib" "$target/$profile/libonnxruntime.dylib"
    chmod 0644 "$target/$profile/libonnxruntime.dylib"
    echo "$target/$profile/libonnxruntime.dylib"
done
