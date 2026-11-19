# Buildroot image

The canonical rv-emu-rs Linux boot is the **partitioned virtio-blk disk**:

- OpenSBI `fw_dynamic.bin` starts U-Boot.
- U-Boot loads `Image` from a 64 MiB ext2 boot partition.
- Linux mounts a 128 MiB ext4 root filesystem from the second partition.
- U-Boot's compiled-in default environment performs the whole boot; no prompt
  commands are needed.

| Property | Value |
| --- | --- |
| Defconfig | `rv-emu-rs_defconfig` |
| Boot chain | `fw_dynamic.bin` + `u-boot.bin` via `--uboot` |
| Boot partition | 64 MiB ext2, `/Image` |
| Root filesystem | 128 MiB ext4 on `/dev/vda2` |

The image includes BusyBox, PrBoom with the Doom shareware WAD, the framebuffer
test tools, ALSA, and the out-of-tree `primitive-audio` driver.

> **Prebuilt archives live in `images/`.** Unpack them there as shown; the
> extracted `.bin`/`.img` files are gitignored, so only the archives are
> tracked.

## Contents

- [Canonical disk image](#canonical-disk-image)
- [U-Boot default environment](#u-boot-default-environment)
- [Repository layout](#repository-layout)
- [Configuration](#configuration)
- [Technical details](#technical-details)
- [Build and maintenance notes](#build-and-maintenance-notes)

## Canonical disk image

### Build

The shareware WAD is not tracked by git. Copy it into the overlay once, from the
emulator repository root:

```sh
cp doom1.wad buildroot/board/rv-emu-rs/rootfs-overlay/usr/share/games/doom/
```

Then build the disk image in its own output directory:

```sh
REPO=$PWD
BUILDROOT=/path/to/buildroot-2026.08.1

cd "$BUILDROOT"
make O=output-disk BR2_EXTERNAL="$REPO/buildroot" rv-emu-rs_defconfig
make O=output-disk -j"$(nproc)"
```

The disk configuration builds the kernel and a 128 MiB ext4 root filesystem, then `board/rv-emu-rs/post-image-disk.sh` creates the ext2 boot
partition and assembles the disk with `genimage`.

### Layout

| Partition | Type | Size | Filesystem | Contents |
| --- | --- | --- | --- | --- |
| 1, bootable | `0x83` | 64 MiB | ext2 | `/Image` |
| 2 | `0x83` | 128 MiB | ext4 | root filesystem |

Partitions are 1 MiB aligned. The kernel command line gets
`root=/dev/vda2 rootwait rw` from U-Boot.

### Run

From freshly built artifacts:

```sh
cd "$REPO"
cargo run --profile maxspeed -- --uboot \
    "$BUILDROOT/output-disk/images/fw_dynamic.bin" \
    "$BUILDROOT/output-disk/images/u-boot.bin" \
    --disk "$BUILDROOT/output-disk/images/rv-emu-rs-disk.img"
```

Or from the prebuilt archives:

```sh
tar -xzf images/uboot-boot.tar.gz -C images
tar -xzf images/rv-emu-rs-disk.tar.gz -C images

cargo run --profile maxspeed -- --uboot \
    images/fw_dynamic.bin images/u-boot.bin \
    --disk images/rv-emu-rs-disk.img
```

For a headless serial login, add `--no-display`; the emulator selects
`rv-emu-rs-sv32-nodisplay.dtb`, and U-Boot copies the kernel command line and its
`console=` order from that tree's `/chosen/bootargs`. Pass a device tree as the
third path to override the default.

### Artifacts and archives

`output-disk/images/` (disk build):

| File | Purpose |
| --- | --- |
| `rv-emu-rs-disk.img` | complete 193 MiB disk image |
| `Image` | kernel, 31,065,600 bytes |
| `boot.ext2` | 64 MiB ext2 boot partition |
| `rootfs.ext2` / `rootfs.ext4` | 128 MiB ext4 root filesystem |
| `u-boot.bin` | U-Boot with the auto-boot environment |
| `fw_dynamic.bin` | OpenSBI dynamic firmware |

Canonical prebuilt archives in `images/`:

| Archive | Contents |
| --- | --- |
| `uboot-boot.tar.gz` | `fw_dynamic.bin` and `u-boot.bin` |
| `rv-emu-rs-disk.tar.gz` | the partitioned disk image |

## U-Boot default environment

U-Boot is built from the `qemu-riscv32_smode` defconfig, linked at
`0x80400000`, and launched by OpenSBI `fw_dynamic.bin`. The default environment
in `board/rv-emu-rs/uboot.env` is compiled into `u-boot.bin`, so the disk boots
without manual commands:

```text
kernel_addr_r=0x82000000
stdin=serial
stdout=serial,vidconsole
stderr=serial,vidconsole
bootcmd=fdt addr ${fdtcontroladdr}; fdt get value bootargs /chosen bootargs; setenv bootargs ${bootargs} root=/dev/vda2 rootwait rw; virtio scan; ext2load virtio 0:1 ${kernel_addr_r} /Image; booti ${kernel_addr_r} - ${fdtcontroladdr}
```

The boot command:

1. selects the control DTB that OpenSBI passed in (`${fdtcontroladdr}`),
2. copies its `/chosen/bootargs` into the environment and appends
   `root=/dev/vda2 rootwait rw`,
3. scans the virtio bus, loads `/Image` from partition 1 at `0x82000000`,
4. boots it with the same DTB.

Because the console order comes from the DTB, the screen tree logs in on the
framebuffer and the `--no-display` tree logs in on the serial port.

Interrupt the two-second autoboot to get the `=>` prompt. The equivalent manual
commands are:

```text
virtio scan
ext2load virtio 0:1 0x82000000 /Image
booti 0x82000000 - ${fdtcontroladdr}
```

Notes:

- The boot log should contain `devicetree: board`; that confirms
  `CONFIG_OF_BOARD` used the DTB from OpenSBI instead of U-Boot's built-in
  `qemu-virt32` tree.
- OpenSBI reserves `[0x80000000-0x8004ffff]` (`no-map` in `bdinfo`); do not load
  the kernel there.
- U-Boot's stock `kernel_addr_r=0x84000000` can overlap its relocated image, so
  the default environment uses `0x82000000`.
- U-Boot's stock `fdt_addr_r=0x8c000000` is outside the 128 MiB guest, so the
  default environment uses `${fdtcontroladdr}`.
- The emulator advertises a 4 MiB `size_max` and 16 segments (64 MiB per
  request), so U-Boot can load the kernel in one chain.

## Repository layout

| Path | Purpose |
| --- | --- |
| `buildroot/external.desc` | br2-external tree name, `RVEMURS` |
| `buildroot/configs/rv-emu-rs_defconfig` | canonical partitioned-disk build |
| `buildroot/board/rv-emu-rs/32-bit.config` | copy of the kernel's 32-bit fragment |
| `buildroot/board/rv-emu-rs/linux.fragment` | board kernel options |
| `buildroot/board/rv-emu-rs/uboot.env` | U-Boot default environment |
| `buildroot/board/rv-emu-rs/post-image-disk.sh` | boot partition and disk assembly |
| `buildroot/board/rv-emu-rs/genimage-disk.cfg` | MBR layout for the disk image |
| `buildroot/package/primitive-audio/` | out-of-tree ALSA driver |
| `buildroot/board/rv-emu-rs/rootfs-overlay/` | `/root/doom`, demo scripts, config, WAD |

## Configuration

### Target and toolchain

| Option | Why |
| --- | --- |
| `BR2_riscv=y`, `BR2_RISCV_32=y` | 32-bit RISC-V |
| `BR2_RISCV_ISA_RVC=y` | compressed instructions; I/M/A/F/D come from the default G core, MMU is on by default |
| `BR2_TOOLCHAIN_BUILDROOT_MUSL=y` | musl, internal toolchain |
| `BR2_BINUTILS_VERSION_2_46_X=y`, `BR2_GCC_VERSION_16_X=y` | pin 2.46.1 / 16.2.0 |

### Kernel

| Option | Why |
| --- | --- |
| `BR2_LINUX_KERNEL=y` | build the kernel |
| `BR2_LINUX_KERNEL_USE_ARCH_DEFAULT_CONFIG=y` | expand `arch/riscv/configs/defconfig` |
| `BR2_LINUX_KERNEL_CONFIG_FRAGMENT_FILES=...` | merge `32-bit.config` and `linux.fragment` |
| `BR2_LINUX_KERNEL_NEEDS_HOST_OPENSSL=y` | selected by kernel crypto |

Linux 7.1 has no `rv32_defconfig`: 32-bit is the generic `defconfig` plus
`arch/riscv/configs/32-bit.config`. The board keeps a copy of that fragment and
adds `linux.fragment`:

```text
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

The generic RISC-V defconfig already provides the framebuffer console,
devtmpfs mounting, `VIRTIO_MMIO`, `VIRTIO_BLK`, the PLIC and the RISC-V timer.
When bumping the kernel, refresh `32-bit.config` and re-check the fragment.

### Userspace

| Option | Why |
| --- | --- |
| `BR2_PACKAGE_PRBOOM=y` | Doom, with SDL's fbcon backend |
| `BR2_PACKAGE_FB_TEST_APP=y` | `fb-test` tools |
| `BR2_PACKAGE_ALSA_LIB=y` | ALSA userspace; SDL uses its ALSA backend |
| `BR2_PACKAGE_PRIMITIVE_AUDIO=y` | out-of-tree PCM driver for the emulator audio device |
| `BR2_PACKAGE_COREMARK=y`, `BR2_PACKAGE_STRESS_NG=y` | benchmarks |

### Disk build

| Option | Why |
| --- | --- |
| `BR2_TARGET_ROOTFS_EXT2=y`, `BR2_TARGET_ROOTFS_EXT2_4=y`, `BR2_TARGET_ROOTFS_EXT2_SIZE="128M"` | build `rootfs.ext4` |
| `BR2_PACKAGE_HOST_GENIMAGE=y` | assemble the partition table and disk |
| `BR2_ROOTFS_POST_IMAGE_SCRIPT=.../post-image-disk.sh` | create `boot.ext2` and run genimage |

### OpenSBI and U-Boot

| Option | Why |
| --- | --- |
| `BR2_TARGET_UBOOT=y`, `BR2_TARGET_UBOOT_CUSTOM_VERSION_VALUE="2026.07"` | pin U-Boot |
| `BR2_TARGET_UBOOT_BOARD_DEFCONFIG="qemu-riscv32_smode"` | S-mode payload linked at `0x80400000` |
| `BR2_TARGET_UBOOT_NEEDS_DTC=y` | U-Boot device-tree compiler |
| `BR2_TARGET_UBOOT_DEFAULT_ENV_FILE="$(BR2_EXTERNAL_RVEMURS_PATH)/board/rv-emu-rs/uboot.env"` | compile in the auto-boot environment |
| `BR2_TARGET_UBOOT_FORMAT_BIN=y` | install `u-boot.bin` |
| `BR2_TARGET_OPENSBI=y`, `BR2_TARGET_OPENSBI_PLAT="generic"` | OpenSBI 1.8.1 |
| `BR2_TARGET_OPENSBI_INSTALL_DYNAMIC_IMG=y` | `fw_dynamic.bin` for `--uboot` |

## Technical details

### Audio

`buildroot/package/primitive-audio/` exposes the emulator's audio device as an
ALSA playback PCM (stereo, S16_LE). The device has two MMIO registers at
`0x10000200`:

| Offset | Access | Meaning |
| --- | --- | --- |
| `0x0` | write halfword | enqueue one signed 16-bit sample |
| `0x0` | read word | samples still queued |
| `0x4` | read/write word | sample rate in Hz (8000-192000, default 44100) |

The driver advertises the continuous rate range and writes the application's
rate during `hw_params`. It pushes one period per hrtimer tick. The host audio
source reads the register and retunes playback, so `aplay -r 48000` changes the
actual rate. The module is loaded by `/etc/init.d/S40primitive-audio`
(`modprobe primitive_audio`); PrBoom uses SDL's ALSA backend. `DOOM_SOUND=0
./doom` passes `-nosound` when the host has no audio device.

### Input

The emulator exposes one Goldfish events device at `0x10001000` (PLIC source
11), compatible with Linux's `google,goldfish-events-keypad` driver. It provides
keyboard keys, mouse buttons, and absolute `ABS_X`/`ABS_Y` axes. Pointer motion
is absolute, so the guest cursor follows the host cursor without a grab.

SDL 1.2's fbcon backend reads `/dev/input/mice`, served by `mousedev`; its
absolute-device path converts coordinates to the relative packets SDL expects.
`CONFIG_INPUT_MOUSEDEV_SCREEN_X/Y` are 640x480, matching the framebuffer.
Because the goldfish driver syncs every three words, the first report after a
client opens can include a one-time centre correction; vanilla Doom only uses
the X axis, so turning is unaffected.

Keyboard and button events are delivered only while the window has focus;
losing focus releases held keys and buttons. Mouse buttons share the input
device with the keyboard, so the VT keyboard handler sees them as extended
scancodes in `K_MEDIUMRAW`; SDL parses a few spurious releases, which Doom
ignores.

### Device trees and consoles

`rv-emu-rs-sv32.dts` lists `tty0` last, so the framebuffer VT is the preferred
console and the login appears in the emulator window. `rv-emu-rs-sv32-nodisplay.dts`
lists `ttyS0` last, so `--no-display` keeps the serial console as the login.
Both register both ports, so kernel and `earlycon` messages reach the UART either
way.

## Build and maintenance notes

### Build directory

`BR2_EXTERNAL` is remembered in the output directory's `.br2-external.mk`, so
later plain `make` invocations keep using this tree. Pass it again only when
creating a new output directory.

### Cache and host tools

- If the Buildroot checkout has no `dl/`, symlink a previous checkout's `dl/`
  into it, or let Buildroot populate it.
- Host QEMU (`BR2_PACKAGE_HOST_QEMU`) is not enabled; it does not change the
  target image.
- `doom1.wad` is not tracked by git (see `.gitignore`); without it the image
  still boots, but `doom` has no WAD.

### Refreshing the defconfig

After `menuconfig`, save the configuration back to the external tree:

```sh
cd "$BUILDROOT"
make O=output-disk BR2_EXTERNAL="$REPO/buildroot" savedefconfig \
    DEFCONFIG="$REPO/buildroot/configs/rv-emu-rs_defconfig"
```

`savedefconfig` keeps `$(BR2_EXTERNAL_RVEMURS_PATH)` in stored paths and drops
comments.
