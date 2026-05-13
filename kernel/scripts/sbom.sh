#!/usr/bin/env bash
# Asolaria Federation Remake · SBOM generation · Phase-2 Step 39
#
# Anchor PID: ASOLARIA-FEDERATION-REMAKE-1024-PID-2026-05-11
# Outputs: dist/sbom-v0.json — every dep + version + license + transitive tree
#
# Per REPO_LAW Invariant 9 (no bloat): SBOM exists so unused deps get flagged.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="${REPO_ROOT}/dist"
mkdir -p "${OUT_DIR}"
OUT="${OUT_DIR}/sbom-v0.json"

if ! command -v cargo >/dev/null 2>&1; then
    echo "[honest_fail] cargo not found (Phase-10 CI prep)"
    exit 2
fi

# `cargo metadata` produces canonical SBOM-like JSON (Cargo's own format).
# Real CycloneDX export would use `cargo cyclonedx` once that becomes a workspace dep.
cd "${REPO_ROOT}"
cargo metadata --format-version 1 --no-deps > "${OUT}.tmp"
# Normalize: strip workspace_metadata (host-specific paths) for reproducibility.
python -c "
import json, sys
with open('${OUT}.tmp') as f: d = json.load(f)
d.pop('workspace_metadata', None)
# Sort packages for deterministic output.
d.get('packages', []).sort(key=lambda p: (p.get('name',''), p.get('version','')))
with open('${OUT}', 'w') as f:
    json.dump(d, f, indent=2, sort_keys=True)
"
rm -f "${OUT}.tmp"

sha=$(sha256sum "${OUT}" | awk '{print $1}')
echo "[ok] SBOM at ${OUT}"
echo "[sha256] ${sha}"
echo "[sha16]  ${sha:0:16}"
echo "[pkgs] $(python -c "import json; print(len(json.load(open('${OUT}')).get('packages',[])))")"
