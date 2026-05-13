#!/usr/bin/env bash
# Asolaria Federation Remake · Kernel image build · Phase-2 Step 37
#
# Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
# Multi-arch (ARM64-priority-1, x86_64-priority-2) per KERNEL_TARGETS.md
# Reproducibility per Phase-2 Step 38 (3 independent builds → identical sha)
#
# Usage:
#   ./build-img.sh aarch64   # ARM64 (Falcon/Aether)
#   ./build-img.sh x86_64    # x86_64 (Acer/Liris)
#   ./build-img.sh both      # build both arches sequentially
#
# Exit codes:
#   0  build OK, sha logged
#   1  unknown arch
#   2  cargo missing (acer-host pre-Phase-10 — script honest-fails)
#   3  build failure

set -euo pipefail

ARCH="${1:-both}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${REPO_ROOT}/target"
OUT_DIR="${REPO_ROOT}/dist"
mkdir -p "${OUT_DIR}"

# Phase-2 honest-fail: cargo not yet installed on acer-host (Step 38 prep)
if ! command -v cargo >/dev/null 2>&1; then
    echo "[honest_fail] cargo not found on this host (acer-host pre-Phase-10 CI)"
    echo "[expected_path] this script runs green once cargo + rustup target-add aarch64-unknown-uefi land in Phase-10 Step 181 (CI workflow)"
    exit 2
fi

build_arch() {
    local target_triple="$1"
    local label="$2"
    echo "[build] arch=${label} triple=${target_triple}"
    cd "${REPO_ROOT}"
    cargo build --release --target "${target_triple}" --bin asolaria-os
    local artifact="${TARGET_DIR}/${target_triple}/release/asolaria-os.efi"
    if [[ ! -f "${artifact}" ]]; then
        echo "[fail] artifact not produced at ${artifact}"
        return 3
    fi
    local sha
    sha="$(sha256sum "${artifact}" | awk '{print $1}')"
    local sha16="${sha:0:16}"
    local dest="${OUT_DIR}/asolaria-os-${label}.efi"
    cp "${artifact}" "${dest}"
    echo "[ok] ${label} sha256=${sha} sha16=${sha16} → ${dest}"
}

case "${ARCH}" in
    aarch64)
        build_arch "aarch64-unknown-uefi" "aarch64"
        ;;
    x86_64)
        build_arch "x86_64-unknown-uefi" "x86_64"
        ;;
    both)
        build_arch "aarch64-unknown-uefi" "aarch64"
        build_arch "x86_64-unknown-uefi" "x86_64"
        ;;
    *)
        echo "[fail] unknown arch: ${ARCH}"
        echo "[usage] $0 {aarch64|x86_64|both}"
        exit 1
        ;;
esac

echo "[done] artifacts in ${OUT_DIR}/"
ls -la "${OUT_DIR}/"
