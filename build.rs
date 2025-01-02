fn main() {
    let file = "src/platform/SNEForwardingFilePromiseProvider.mm";
    let mut b = cc::Build::new();
    b.file(file).flag("-fobjc-arc").cpp(true).compile("SNEForwardingFilePromiseProvider");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=AppKit");

    println!("cargo:rerun-if-changed={}", file);
    println!("cargo:rerun-if-changed=build.rs");
}
