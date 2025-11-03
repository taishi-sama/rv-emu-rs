use std::{env, fs::File, io::{BufReader, BufWriter, Write}, path::PathBuf};
const AUDIO: &str = "./Bad Apple!!.ogg";
fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Put the linker script somewhere the linker can find it.
    //fs::write(out_dir.join("memory.x"), include_bytes!("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=build.rs");
    
    let file = BufReader::new(File::open(AUDIO).unwrap());
    let source = rodio::Decoder::new(file).unwrap();
    let samples: Vec<i16> = source.map(|x|(x * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16).collect();
    // panic!("samples: {} ({} bytes)", samples.len(), samples.len() * 2);
    let mut fl = BufWriter::new(File::create("./raw_audio_stream.bin").unwrap());
    for s in samples {
        let v: [u8; 2] = s.to_le_bytes();
        fl.write(&v).unwrap();
    }
    // writeln!(fl, "{:#?}", samples).unwrap();
}