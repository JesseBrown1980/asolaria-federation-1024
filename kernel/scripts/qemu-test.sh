#!/usr/bin/env bash
# Asolaria Federation Remake · QEMU testbed · Phase-2 Step 24
#
# Boots the built .efi under QEMU + OVMF firmware for fast dev-loop iteration.
# Reproducibility per Phase-2 Step 38.
#
# Usage:
#   ./qemu-test.sh aarch64
#   ./qemu-test.sh x86_64
#
# Pre-req: build-img.sh has produced dist/asolaria-os-<arch>.efi

set -euo pipefail

ARCH="${1:-x86_64}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="${REPO_ROOT}/dist"
ARTIFACT="${DIST}/asolaria-os-${ARCH}.efi"

if [[ ! -f "${ARTIFACT}" ]]; then
    echo "[fail] artifact missing at ${ARTIFACT}"
    echo "[hint] run ./build-img.sh ${ARCH} first"
    exit 1
fi

case "${ARCH}" in
    aarch64)
        if ! command -v qemu-system-aarch64 >/dev/null 2>&1; then
            echo "[honest_fail] qemu-system-aarch64 not installed (Phase-10 CI prep)"
            exit 2
        fi
        # OVMF for ARM64
        OVMF_CODE="${OVMF_CODE:-/usr/share/qemu-efi-aarch64/QEMU_EFI.fd}"
        qemu-system-aarch64 \
            -machine virt -cpu cortex-a72 -m 512 -nographic \
            -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
            -drive format=raw,file=fat:rw:"$(dirname "${ARTIFACT}")"
        ;;
    x86_64)
        if ! command -v qemu-system-x86_64 >/dev/null 2>&1; then
            echo "[honest_fail] qemu-system-x86_64 not installed (Phase-10 CI prep)"
            exit 2
        fi
        OVMF_CODE="${OVMF_CODE:-/usr/share/OVMF/OVMF_CODE.fd}"
        qemu-system-x86_64 \
            -machine q35 -m 512 -nographic \
            -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
            -drive format=raw,file=fat:rw:"$(dirname "${ARTIFACT}")"
        ;;
    *)
        echo "[fail] unknown arch: ${ARCH}"
        exit 1
        ;;
esac
