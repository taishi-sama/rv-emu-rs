# Buildroot image

The emulator boots Linux built by Buildroot: a single OpenSBI `fw_payload.bin`
(OpenSBI + kernel) with an initramfs containing BusyBox, PrBoom and the Doom
shareware WAD.

## Build

Download Buildroot 2026.08.1 archive. The shareware WAD is not tracked by git, so put
it in the overlay first (once), from the emulator repository root:

```sh
cp doom1.wad buildroot/board/rv-emu-rs/rootfs-overlay/usr/share/games/doom/
```

Then, with the repository and Buildroot checkout locations in two variables:

```sh
REPO=$PWD                        # rv-emu-rs checkout
BUILDROOT=/path/to/buildroot-2026.08.1

cd "$BUILDROOT"
make BR2_EXTERNAL="$REPO/buildroot" rv-emu-rs_defconfig
make -j"$(nproc)"
make opensbi-rebuild
```

## Run 
```sh
cd "$REPO"
cargo run --profile maxspeed -- --payload \
  "$BUILDROOT/output/images/fw_payload.bin"
```

## Technical build details


`BR2_EXTERNAL` is remembered in `output/.br2-external.mk`, so later plain `make`
invocations keep using this tree. It only has to be passed again when running
`rv-emu-rs_defconfig` into a new `output/`.

**`make opensbi-rebuild` is required on a clean tree.** The kernel is linked
before the rootfs exists, with a zero-byte placeholder for `rootfs.cpio`, and
OpenSBI is built during `target-finalize`, before the real cpio is generated.
Buildroot does rebuild `Image` after creating `rootfs.cpio`, but nothing
rebuilds `fw_payload.bin`, so the first pass leaves a payload with an empty
initramfs. Run `make opensbi-rebuild` again after any change to the overlay or
the kernel configuration too.

After changing `BR2_TARGET_OPENSBI_ADDITIONAL_VARIABLES` in an already-built
tree, run `make opensbi-dirclean` first. `opensbi-rebuild` re-runs OpenSBI's
make, but OpenSBI does not track compiler flags, so existing objects are reused
and the new `-D` value is silently ignored.

Artifacts in `output/images/`:

- `fw_payload.bin` — the one to boot, 43,648,520 bytes (41.6 MiB)
- `Image` — kernel alone, for the `fw_dynamic.bin` boot path
- `fw_dynamic.bin`, `fw_jump.bin` — built too, unused
- `rootfs.cpio` — the initramfs that is embedded in `Image`

## Run


## Description

Everything board-specific lives in this directory, which is used as a Buildroot
`BR2_EXTERNAL` tree. The Buildroot checkout itself only gets `.config`,
`output/` and optionally a `dl` symlink; nothing in it is patched.

## Layout

| Path | Purpose |
| --- | --- |
| `buildroot/external.desc` | br2-external tree name, `RVEMURS` |
| `buildroot/configs/rv-emu-rs_defconfig` | the complete board configuration |
| `buildroot/board/rv-emu-rs/32-bit.config` | copy of the kernel's 32-bit fragment |
| `buildroot/board/rv-emu-rs/linux.fragment` | board kernel options (simplefb, logo) |
| `buildroot/board/rv-emu-rs/rootfs-overlay/` | `/root/doom`, demo scripts, prboom config, WAD |


## OpenSBI FDT placement

`fw_payload` relocates the FDT before entering the kernel. By default the
destination is `FW_PAYLOAD_FDT_OFFSET = 0x2200000`, i.e. physical `0x82200000`,
which is **inside the 39,454,208-byte kernel image** (`0x80400000..0x829A0608`).
The kernel is linked at `0xC0000000`, so that physical address is virtual
`0xC1E00000` — the start of a `.rodata` block holding `"TRAILER!!!"`,
`"070701"`, `"no cpio magic"` and every `unpack_to_rootfs()` error string.

The symptoms are confusing: a stray `phandle` line in the boot log (a
`pr_info()` format string replaced by the FDT string table) and
`Kernel panic - not syncing:` with an **empty** message in
`do_populate_rootfs()`, because the cpio parser's magic/trailer strings and its
error messages were overwritten.

The defconfig therefore sets:

```
BR2_TARGET_OPENSBI_ADDITIONAL_VARIABLES="FW_PAYLOAD_FDT_ADDR=0x87FFC000"
```

`0x87FFC000` is the emulator's `DTB_ADDRESS` (`src/emulator.rs`). With the
destination equal to the source, OpenSBI skips the copy entirely and the kernel
receives the same FDT pointer as on the `fw_dynamic` path. If `DTB_ADDRESS`
changes, update this variable as well. Any address above the payload and below
the top of RAM works; keeping it equal to `DTB_ADDRESS` avoids the copy.

Changing this variable in an already-built tree requires
`make opensbi-dirclean` before `make opensbi-rebuild`; see the build section.

## Configuration

### Target and toolchain

| Option | Why |
| --- | --- |
| `BR2_riscv=y`, `BR2_RISCV_32=y` | 32-bit RISC-V |
| `BR2_RISCV_ISA_RVC=y` | enable compressed instructions; I/M/A/F/D come from the default G core selection (`BR2_riscv_g`), and the MMU is on by default |
| `BR2_TOOLCHAIN_BUILDROOT_MUSL=y` | musl, internal toolchain |
| `BR2_BINUTILS_VERSION_2_46_X=y`, `BR2_GCC_VERSION_16_X=y` | pin the versions the kernel config was generated with (2.46.1 / 16.2.0) |

### Board files

| Option | Why |
| --- | --- |
| `BR2_ROOTFS_OVERLAY="$(BR2_EXTERNAL_RVEMURS_PATH)/board/rv-emu-rs/rootfs-overlay"` | Doom scripts and WAD |
| `BR2_LINUX_KERNEL=y` | build the kernel |
| `BR2_LINUX_KERNEL_USE_ARCH_DEFAULT_CONFIG=y` | expand `arch/riscv/configs/defconfig` |
| `BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES=...` | merge `32-bit.config` and `linux.fragment` into it |
| `BR2_LINUX_KERNEL_NEEDS_HOST_OPENSSL=y` | kernel crypto needs host OpenSSL |

Since Linux 7.1 there is no `rv32_defconfig`: 32-bit is the generic `defconfig`
plus `arch/riscv/configs/32-bit.config`. The board keeps a copy of that
fragment and adds `linux.fragment`, which only sets:

```
CONFIG_FB_SIMPLE=y
CONFIG_LOGO=y
CONFIG_LOGO_LINUX_CLUT224=y
# CONFIG_GCC_PLUGINS is not set
```

Everything else the image needs is already the default:

- `CONFIG_FRAMEBUFFER_CONSOLE=y`, `CONFIG_DEVTMPFS*_MOUNT`,
  `CONFIG_VIRTIO_MMIO`, `CONFIG_VIRTIO_BLK`, `CONFIG_SIFIVE_PLIC`,
  `CONFIG_RISCV_TIMER`
- `CONFIG_INITRAMFS_SOURCE` is injected by Buildroot when
  `BR2_TARGET_ROOTFS_INITRAMFS=y`, and the compression choice defaults to gzip
  (~10.6 MiB embedded for the 25 MiB cpio)
- `# CONFIG_DRM_SIMPLEDRM is not set` by default

When bumping the kernel, refresh `32-bit.config` from the new
`arch/riscv/configs/32-bit.config` and re-check the fragment.

### Userspace

| Option | Why |
| --- | --- |
| `BR2_PACKAGE_PRBOOM=y` | Doom, pulls in SDL with the fbcon backend |
| `BR2_PACKAGE_FB_TEST_APP=y` | `fb-test`, `fb-test-perf`, `fb-test-rect`, `fb-test-offset`, `fb-test-string` |
| `BR2_PACKAGE_COREMARK=y`, `BR2_PACKAGE_STRESS_NG=y` | benchmarks, kept from the previous image |

### Boot image

| Option | Why |
| --- | --- |
| `BR2_TARGET_ROOTFS_INITRAMFS=y` | embed `rootfs.cpio` in the kernel image |
| `# BR2_TARGET_ROOTFS_TAR is not set` | don't build an unused tarball |
| `BR2_TARGET_OPENSBI=y`, `BR2_TARGET_OPENSBI_PLAT="generic"` | OpenSBI 1.8.1, generic platform |
| `BR2_TARGET_OPENSBI_LINUX_PAYLOAD=y` | one `fw_payload.bin`; also selects `BR2_TARGET_OPENSBI_INSTALL_PAYLOAD_IMG` |
| `BR2_TARGET_OPENSBI_ADDITIONAL_VARIABLES="FW_PAYLOAD_FDT_ADDR=0x87FFC000"` | keep the FDT out of the kernel image (see above) |

With `FW_PAYLOAD_OFFSET = 0x400000` for XLEN=32 and the kernel's boot header
`text_offset = 0x400000`, the payload is self-consistent: OpenSBI sits at
`0x80000000`, the kernel entry is `0x80400000`. No `FW_JUMP_*` variables are
needed.

## Notes

- If the Buildroot checkout has no `dl/` directory, symlink a previous checkout's
  `dl/` into it to reuse the download cache, or let Buildroot populate it.
- Host QEMU (`BR2_PACKAGE_HOST_QEMU`) is not enabled: it only builds a host tool
  and does not change the target image.
- The initramfs is a plain (uncompressed) cpio on disk; the kernel compresses it
  when embedding it (`CONFIG_INITRAMFS_COMPRESSION_GZIP=y`). Setting
  `BR2_TARGET_ROOTFS_CPIO_GZIP=y` only adds a sibling `rootfs.cpio.gz` and would
  not shrink the payload.
- `doom1.wad` is not tracked by git (see `.gitignore`). Without it the image
  still boots, but `doom` has no WAD to load.
- To refresh the defconfig after `menuconfig`:

  ```sh
  cd "$BUILDROOT"
  make BR2_EXTERNAL="$REPO/buildroot" savedefconfig \
    DEFCONFIG="$REPO/buildroot/configs/rv-emu-rs_defconfig"
  ```

  `savedefconfig` keeps `$(BR2_EXTERNAL_RVEMURS_PATH)` in the stored paths,
  drops comments, and adds `BR2_TARGET_OPENSBI_INSTALL_PAYLOAD_IMG=y`.
