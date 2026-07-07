# Asolaria QEMU/OVMF Boot Proof - 2026-07-06

CLAIM|text=Existing Asolaria x86_64 ESP boots under WSL QEMU plus OVMF to the ASOLARIA ASI OS banner.
EVIDENCE|class=MEASURED_ACER_WSL|surface=qemu-system-x86_64|version=QEMU emulator version 8.2.2 (Debian 1:8.2.2+ds-0ubuntu1.16)
EVIDENCE|class=MEASURED_ACER_WSL|surface=OVMF|path=/usr/share/OVMF/OVMF_CODE_4M.fd|sha256=3a23911c0b32be25fc3eae579b0b7197d8f321b6f0d6cb16aac25c67f3ec712b
EVIDENCE|class=MEASURED_ACER|surface=boot_log|path=docs/receipts/ASOLARIA-QEMU-OVMF-BOOT-PROOF-2026-07-06.boot-log.txt|sha256=ebfb4d0720ca50f9ba41d69954168e5c18460aad3a2fbc1af1bfbc033c0160ed
EVIDENCE|class=MEASURED_ACER|surface=efi_artifact|sha256=a38f48f3adb39a4b3d57daf03d2980b0964f0dd509274a84eac61f66a0dcfdfa|bytes=11264
EVIDENCE|class=MEASURED_ACER|surface=esp_artifact|sha256=f1323a5dbbbbdc035011e89dc1a05a729f62299c550c9383098f17ae0d25b7a4|bytes=100663296
BOUNDARY|class=IMPORTANT|detail=This is WSL emulated boot proof, not physical USB boot proof. No USB write, no format, no boot-entry edit.

## Observed Boot Banner

- `ASOLARIA ASI OS . kernel 0.2.0-phase3-scaffold . booting`
- `federation-1024 . envelope-REPL init . E=0 . fire=0`

## Interpretation

The previous blocker “QEMU/OVMF not installed on Acer” is now split: Windows-native QEMU is absent, but WSL Ubuntu on Acer has QEMU 8.2.2 and OVMF. The existing ESP image boots far enough to print the Asolaria banner. Physical USB boot-menu visibility remains unproven.
