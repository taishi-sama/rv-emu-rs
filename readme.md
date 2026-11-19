

# What is it.
My small personal project targeted to write RISC-V emulator with decent accuracy.

Current target instruction set is rv32imafdc, with Sv32 virtual memory and user mode - essentially full RV32GC.

Devices list contains a 16550-compatible UART, a stereo 16-bit audio device with a programmable sample rate, a simple 640x480 framebuffer, a Goldfish events keyboard/absolute-pointer input device, a PLIC, a virtio-mmio block device, a CLINT and a syscon. Will be extended in the future.

# DOOM SHOWCASE
https://github.com/user-attachments/assets/0e2e8eab-3d41-4dde-a272-1f81b0bc895d

## Old Bad Apple showcase

https://github.com/user-attachments/assets/7547b81b-a22c-4d94-8c82-70087b4962d8



## Buildroot Linux example

Emulator is fully capable of running practically vanilla buildroot Linux images with minor adjustments in form of additionally enabled drivers and image layout configs.

In case of pre-downloaded image:
Unpack archived image, e.g.:
```sh
tar -xzf images/fw_payload.bin.tar.gz -C images
```
then run it
```sh
cargo run --profile maxspeed -- --payload images/fw_payload.bin
```

And log in on the emulator window (the framebuffer VT is the preferred console),
then run either

```sh
./doom_demo.sh
```

```sh
./doom_timedemo.sh
```
or

```sh
coremark
```

The serial port receives kernel and `earlycon` messages in both cases; the
preferred console (and therefore the login shell) follows the selected device
tree. `--no-display` selects the serial-console variant automatically.

Full instruction how to build buildroot from scratch is described in [BUILDROOT.md](./BUILDROOT.md)
Essentially just run download buildroot, (optionally) download doom1.wad for PRBOOM that runs inside emulator, place according instruction, run described commands.

# Running

```sh
cargo run --profile maxspeed -- --payload images/fw_payload.bin
```

`rv-emu-rs --help` lists the boot modes and options. Without an explicit DTB the emulator
uses `rv-emu-rs-sv32.dtb`, which makes the framebuffer VT the login console. With
`--no-display` it uses `rv-emu-rs-sv32-nodisplay.dtb`, which keeps the serial console as the
login shell, so headless boots remain interactive. Audio is optional: on hosts without a
sound device the emulator keeps running and silently drops samples. The window forwards the
host keyboard and an absolute pointer to the guest; keyboard and button events are active
while the window has focus.

The F and D extensions are built in by default; a strict RV32IMAC build without them is
`cargo run --no-default-features`.

A raw image can be attached as a virtio-blk disk (`/dev/vda`) with:

```sh
truncate -s 64M /tmp/virtio-disk.img
mkfs.ext4 -F -q /tmp/virtio-disk.img
cargo run --profile maxspeed -- --payload images/fw_payload.bin \
    --disk /tmp/virtio-disk.img
```

Add `--disk-ro` to advertise the disk as read-only. The kernel image is expected to have
`CONFIG_VIRTIO_MMIO`, `CONFIG_VIRTIO_BLK` and the filesystem driver built in; the emulator
does not require a PCI bus.

# Developing

Running the test suite requires a `riscv32-elf` binutils toolchain (`riscv32-elf-as`,
`riscv32-elf-ld`) for the assembly arch tests, and `dtc` to rebuild the device trees:

```sh
cargo test
dtc -I dts -O dtb -o rv-emu-rs-sv32.dtb rv-emu-rs-sv32.dts
dtc -I dts -O dtb -o rv-emu-rs-sv32-nodisplay.dtb rv-emu-rs-sv32-nodisplay.dts
```

# Credits 
## Emulation
Project was heavily inspired by [mini-rv32ima emulator](https://github.com/cnlohr/mini-rv32ima).

elf_analyzer.rs and some parts of code was took from [riscv-rust emulator](https://github.com/d0iasm/rvemu). Corresponding license is located in OTHER_LICENSES folder.


## Bad Apple example
Rights to "Bad Apple!!" is owned by Alstroemeria Records. 
bad_apple/Bad Apple!!.ogg was ripped from [this video](https://www.youtube.com/watch?v=FtutLA63Cp8).

Thanks to [Chion82](https://github.com/Chion82/ASCII_bad_apple) for their ASCII-conversion of video clip.


## Doom shareware WAD
DOOM is © 1993 id Software, Inc. `doom1.wad` is the shareware episode's game
data; it is proprietary, not free software, and is not covered by the GPL
source release.

The prebuilt image `images/fw_payload.bin.tar.gz` contains this WAD so the
example boots without extra downloads; redistributing that image redistributes
the shareware data under id Software's shareware terms. The overlay copy in
`buildroot/board/rv-emu-rs/rootfs-overlay/usr/share/games/doom/` is a local
build input and is excluded from git.


## Buildroot
The prebuilt Linux image was built with
[Buildroot](https://buildroot.org/) 2026.08.1, © the Buildroot developers,
distributed under the GNU General Public License version 2 or later (with the
exceptions listed in its `COPYING` file). Buildroot is only the build system;
the image also contains Linux, BusyBox, musl, SDL, PrBoom and OpenSBI, each
under its own license. The exact configuration is in `buildroot/` and is
documented in [BUILDROOT.md](./BUILDROOT.md).

