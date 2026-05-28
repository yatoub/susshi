#!/usr/bin/env bash
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/' | head -1)
TAG="v${VERSION}"

echo "Updating PKGBUILD to ${TAG}..."

curl -fsSL "https://github.com/yatoub/susshi/archive/refs/tags/${TAG}.tar.gz" \
    -o /tmp/susshi-archive.tar.gz
B2SUM=$(b2sum /tmp/susshi-archive.tar.gz | cut -d' ' -f1)

sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD
sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD
sed -i "s/^b2sums=(.*/b2sums=(${B2SUM})/" PKGBUILD

echo "Updating PKGBUILD.bin to ${TAG}..."

curl -fsSL "https://github.com/yatoub/susshi/releases/download/${TAG}/susshi-linux-x86_64" \
    -o /tmp/susshi-linux-x86_64
B2SUM_BIN=$(b2sum /tmp/susshi-linux-x86_64 | cut -d' ' -f1)

sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD.bin
sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD.bin
sed -i "s/^b2sums=(.*/b2sums=('${B2SUM}')/" PKGBUILD.bin
sed -i "s/^b2sums_x86_64=(.*/b2sums_x86_64=('${B2SUM_BIN}')/" PKGBUILD.bin

echo "Updating susshi.spec to ${VERSION}..."
sed -i "s/^Version:.*/Version:        ${VERSION}/" susshi.spec

echo "Done: pkgver=${VERSION}, b2sums=${B2SUM}, b2sums_x86_64=${B2SUM_BIN}"
