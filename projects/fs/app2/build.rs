fn main() {
    println!("cargo:rustc-link-arg=-Tdefault.ld");
    println!("cargo:rustc-link-arg=-Tapp2.ld");
}
