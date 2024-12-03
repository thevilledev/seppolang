fn main() {
    // For macOS, specify system library path
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=native=/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib");
        println!("cargo:rustc-link-lib=System");
        println!("cargo:rustc-link-search=native=/opt/homebrew/opt/zstd/lib");
        println!("cargo:rustc-link-search=native=/opt/homebrew/opt/llvm@18/lib");
        println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
    }
    else {
        panic!("Unsupported OS");
    }
} 