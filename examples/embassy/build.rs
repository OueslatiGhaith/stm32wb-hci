use std::{env, error::Error, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = env::var("OUT_DIR")?;
    fs::write(
        format!("{out_dir}/tl_mbox.x"),
        r#"
MEMORY
{
    RAM_SHARED (xrw) : ORIGIN = 0x20030000, LENGTH = 10K
}

SECTIONS
{
    TL_REF_TABLE (NOLOAD) : { *(TL_REF_TABLE) } >RAM_SHARED
    MB_MEM1      (NOLOAD) : { *(MB_MEM1) } >RAM_SHARED
    MB_MEM2      (NOLOAD) : { _sMB_MEM2 = . ; *(MB_MEM2) ; _eMB_MEM2 = . ; } >RAM_SHARED
}
"#,
    )?;

    println!("cargo:rustc-link-search={out_dir}");
    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Ttl_mbox.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

    Ok(())
}
