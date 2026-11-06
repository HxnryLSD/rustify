fn main() {
    println!("cargo:rustc-link-search=native=native-libs");
    println!("cargo:rustc-link-lib=dylib=asound");
    println!("cargo:rerun-if-changed=native-libs/libasound.so");
}
