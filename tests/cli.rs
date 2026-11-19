use std::process::Command;

fn rv_emu(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rv-emu-rs"))
        .args(args)
        .output()
        .expect("could not run rv-emu-rs")
}

#[test]
fn uboot_requires_a_u_boot_path() {
    for args in [&["--uboot"][..], &["--uboot", "images/fw_dynamic.bin"][..]] {
        let output = rv_emu(args);
        assert!(
            !output.status.success(),
            "rv-emu-rs {args:?} unexpectedly succeeded"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("--uboot takes <fw_dynamic.bin> <u-boot.bin> [dtb]"),
            "unexpected stderr for {args:?}: {stderr}"
        );
    }
}

#[test]
fn uboot_with_two_paths_reads_the_firmware_first() {
    let missing = std::env::temp_dir().join("rv-emu-rs-missing-firmware.bin");
    let _ = std::fs::remove_file(&missing);
    let missing = missing.to_str().expect("temporary path is not UTF-8");

    let output = rv_emu(&["--uboot", missing, missing]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not read the firmware"),
        "two-path --uboot did not take the firmware branch: {stderr}"
    );
}
