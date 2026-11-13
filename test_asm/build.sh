#!/bin/sh
set -e

target_fd="target"
linker_script="link.x"
target_conf_base="-march=rv32ima_zicsr_zifencei"
target_conf_rvc="${target_conf_base}_c"

mkdir -p "${target_fd}"

for entry in ./tests/test*.s
do
    filename="${entry##*/}"
    case "${filename}" in
        testc*) target_conf="${target_conf_rvc}" ;;
        *)      target_conf="${target_conf_base}" ;;
    esac
    riscv32-elf-as ${target_conf} "${entry}" -o "${target_fd}/${filename}.o"
    riscv32-elf-ld "${target_fd}/${filename}.o" -T "${linker_script}" \
        --no-warn-rwx-segments -o "${target_fd}/${filename}.elf"
done
