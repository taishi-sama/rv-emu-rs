#!/bin/sh
# Build the ext2 boot partition and assemble the partitioned disk image.
set -eu

BOARD_DIR="$(dirname "$0")"
BINARIES_DIR="${1:-${BINARIES_DIR}}"
BOOT_DIR="${BUILD_DIR}/boot-partition"
ROOTPATH_TMP="$(mktemp -d)"
trap 'rm -rf "$ROOTPATH_TMP"' EXIT

rm -rf "$BOOT_DIR"
mkdir -p "$BOOT_DIR"
cp "${BINARIES_DIR}/Image" "${BOOT_DIR}/Image"

rm -f "${BINARIES_DIR}/boot.ext2"
"${HOST_DIR}/sbin/mke2fs" -q -t ext2 -F -d "$BOOT_DIR" \
    "${BINARIES_DIR}/boot.ext2" 64M

rm -rf "${BUILD_DIR}/genimage.tmp"
"${HOST_DIR}/bin/genimage" \
    --rootpath "$ROOTPATH_TMP" \
    --tmppath "${BUILD_DIR}/genimage.tmp" \
    --inputpath "${BINARIES_DIR}" \
    --outputpath "${BINARIES_DIR}" \
    --config "${BOARD_DIR}/genimage-disk.cfg"
