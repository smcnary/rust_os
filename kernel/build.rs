fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target == "x86_64-unknown-none" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!("cargo:rustc-link-arg=-T{dir}/linker.ld");
        println!("cargo:rustc-link-arg=-nostdlib");
        println!("cargo:rustc-link-arg=-static");
        println!("cargo:rustc-link-arg=-z");
        println!("cargo:rustc-link-arg=max-page-size=0x1000");
    }
    println!("cargo:rerun-if-changed=linker.ld");
}
