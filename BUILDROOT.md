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
| `buildroot/board/rv-emu-rs/linux.fragment` | board kernel options (simplefb, logo, ALSA, goldfish input) |
| `buildroot/package/primitive-audio/` | out-of-tree ALSA driver for the audio device |
| `buildroot/board/rv-emu-rs/rootfs-overlay/` | `/root/doom`, demo scripts, prboom config, WAD |


## OpenSBI FDT placement

`fw_payload` relocates the FDT before entering the kernel. By default the
destination is `FW_PAYLOAD_FDT_OFFSET = 0x2200000`, i.e. physical `0x82200000`,
which is **inside the kernel image** (the Image starts at `0x80400000` and is
about 39 MB in this configuration).
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
CONFIG_SND=y
CONFIG_GOLDFISH=y
CONFIG_KEYBOARD_GOLDFISH_EVENTS=y
CONFIG_INPUT_MOUSEDEV_SCREEN_X=640
CONFIG_INPUT_MOUSEDEV_SCREEN_Y=480
# CONFIG_GCC_PLUGINS is not set
```

Everything else the image needs is already the default:

- `CONFIG_FRAMEBUFFER_CONSOLE=y`, `CONFIG_DEVTMPFS*_MOUNT`,
  `CONFIG_VIRTIO_MMIO`, `CONFIG_VIRTIO_BLK`, `CONFIG_SIFIVE_PLIC`,
  `CONFIG_RISCV_TIMER`
- `CONFIG_INITRAMFS_SOURCE` is injected by Buildroot when
  `BR2_TARGET_ROOTFS_INITRAMFS=y`, and the compression choice defaults to gzip
- `# CONFIG_DRM_SIMPLEDRM is not set` by default

When bumping the kernel, refresh `32-bit.config` from the new
`arch/riscv/configs/32-bit.config` and re-check the fragment.

### Userspace

| Option | Why |
| --- | --- |
| `BR2_PACKAGE_PRBOOM=y` | Doom, pulls in SDL with the fbcon backend |
| `BR2_PACKAGE_FB_TEST_APP=y` | `fb-test`, `fb-test-perf`, `fb-test-rect`, `fb-test-offset`, `fb-test-string` |
| `BR2_PACKAGE_ALSA_LIB=y` | ALSA userspace; SDL is then built with its ALSA audio backend |
| `BR2_PACKAGE_PRIMITIVE_AUDIO=y` | out-of-tree ALSA PCM driver for the emulator's audio device |
| `BR2_PACKAGE_COREMARK=y`, `BR2_PACKAGE_STRESS_NG=y` | benchmarks, kept from the previous image |

### Audio

`buildroot/package/primitive-audio/` is an out-of-tree kernel module exposing
the emulator's audio device as an ALSA playback PCM: stereo, S16_LE. The device
has two MMIO registers at `0x10000200`:

| Offset | Access | Meaning |
| --- | --- | --- |
| `0x0` | write halfword | enqueue one signed 16-bit sample |
| `0x0` | read word | number of samples still queued |
| `0x4` | read/write word | current sample rate in Hz (8000-192000, resets to 44100) |

The driver advertises a continuous 8000-192000 Hz rate range and writes the
rate the application chooses into the register during `hw_params`. It pushes
one period per hrtimer tick, pacing playback to that rate. The emulator's host
audio source reports the register value and returns a finite frame length, so
rodio re-reads the rate and retunes playback when the guest reprograms the
clock (`aplay -r 48000` changes the actual playback rate).

The module is installed as `primitive_audio.ko` and loaded at boot by
`/etc/init.d/S40primitive-audio` (`modprobe primitive_audio`). PrBoom uses
SDL's default audio output, and SDL picks ALSA because `BR2_PACKAGE_ALSA_LIB`
is enabled.

The `doom` script enables sound by default; `DOOM_SOUND=0 ./doom` passes
`-nosound` (for example when the host has no audio device).

### Input

The emulator exposes one Goldfish events device at `0x10001000` (PLIC source
11), compatible with Linux's `google,goldfish-events-keypad` driver. The host
window forwards physical key presses and pointer motion into it; the kernel
builds a single evdev `input_dev` with ordinary `KEY_*` codes, the usual mouse
buttons and absolute `ABS_X`/`ABS_Y` axes.

The pointer is absolute: `ABS_X`/`ABS_Y` are framebuffer pixel coordinates, so
the guest pointer follows the host cursor without a grab or relative drift.
SDL 1.2's fbcon backend reads `/dev/input/mice`, which is served by `mousedev`;
its absolute-device path converts the coordinates into the relative packets
SDL expects. `CONFIG_INPUT_MOUSEDEV_SCREEN_X/Y` are set to 640x480 so that
conversion is 1:1 with the framebuffer.

Because the goldfish driver syncs after every three-word event, mousedev sees
`ABS_X` and `ABS_Y` in separate reports. Its clients start at the screen centre,
so the first report after a client opens can contain a one-time correction from
the centre; vanilla Doom only uses the X axis, so this does not affect turning.

Keyboard and mouse-button events are only delivered while the window has
keyboard focus. Pointer motion is taken whenever the cursor is over the window.
Losing focus releases all held keys and buttons so Doom does not keep moving or
firing.

Mouse buttons share the input device with the keyboard, so Linux's VT keyboard
handler also receives them. In `K_MEDIUMRAW` (which SDL uses) they appear as
extended scancode bytes, which SDL parses as a few spurious key releases. Doom
ignores those; a split keyboard/pointer device would be needed to remove them
entirely.

The `linux.fragment` additions are:

```
CONFIG_GOLDFISH=y
CONFIG_KEYBOARD_GOLDFISH_EVENTS=y
CONFIG_INPUT_MOUSEDEV_SCREEN_X=640
CONFIG_INPUT_MOUSEDEV_SCREEN_Y=480
```

The fragment sets `CONFIG_GOLDFISH=y` explicitly; the generic RISC-V defconfig
also selects it through `ARCH_VIRT`. `CONFIG_INPUT_EVDEV` and
`CONFIG_INPUT_MOUSEDEV` come from the generic RISC-V defconfig.

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

- Two device trees are provided. `rv-emu-rs-sv32.dts` lists `tty0` last, so the
  framebuffer VT is the preferred console and the login shell appears on the
  emulator window; it is selected when `--no-display` is not given.
  `rv-emu-rs-sv32-nodisplay.dts` lists `ttyS0` last, so `--no-display` keeps the
  serial console as the login shell. Both register both ports, so kernel and
  `earlycon` messages reach the UART either way.
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
