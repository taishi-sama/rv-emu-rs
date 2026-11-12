#!/bin/sh
set -e

target_fd="target"
linker_script="link.x"
target_conf="-march=rv32ima_zicsr_zifencei"

mkdir -p "${target_fd}"

for entry in ./tests/test*.s
do
    filename="${entry##*/}"
    riscv32-elf-as ${target_conf} "${entry}" -o "${target_fd}/${filename}.o"
    riscv32-elf-ld "${target_fd}/${filename}.o" -T "${linker_script}" \
        --no-warn-rwx-segments -o "${target_fd}/${filename}.elf"
done
