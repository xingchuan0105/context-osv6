fn main() {
    if let Ok(key) = std::env::var("KEYGEN_PUBLIC_KEY") {
        println!("cargo:rustc-env=KEYGEN_PUBLIC_KEY={key}");
    }
    tauri_build::build()
}
