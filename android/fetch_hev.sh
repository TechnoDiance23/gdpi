#!/bin/sh
# hev-socks5-tunnel (MIT) kaynağını sabit sürümde android/hev-socks5-tunnel klasörüne indirir.
set -e
cd "$(dirname "$0")"
TAG=2.19.3
SHA=c5584feca8f6df70ee53eb2879e034b849a3d15a
[ -f hev-socks5-tunnel/Android.mk ] && exit 0
git clone -q --depth 1 --recurse-submodules --shallow-submodules -b "$TAG" \
  https://github.com/heiher/hev-socks5-tunnel hev-socks5-tunnel
got=$(git -C hev-socks5-tunnel rev-parse HEAD)
[ "$got" = "$SHA" ] || { echo "hev-socks5-tunnel beklenen sürüm değil: $got"; exit 1; }
