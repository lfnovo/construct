fn main() {
    println!("cargo:rerun-if-env-changed=CONSTRUCT_CHANNEL");
    #[cfg(feature = "desktop")]
    {
        let channel = std::env::var("CONSTRUCT_CHANNEL").unwrap_or_else(|_| "dev".to_string());
        if channel != "dev" && channel != "release" {
            panic!("CONSTRUCT_CHANNEL must be either `dev` or `release`, got `{channel}`");
        }
        println!("cargo:rustc-check-cfg=cfg(construct_release)");
        if channel == "release" {
            println!("cargo:rustc-cfg=construct_release");
        }
        println!("cargo:rustc-env=CONSTRUCT_CHANNEL={channel}");
    }
    #[cfg(feature = "desktop")]
    tauri_build::build()
}
