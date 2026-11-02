fn main() {
    println!("cargo:rerun-if-changed=../../VERSION");
    println!("cargo:rerun-if-changed=../../packaging/icons/Fountage.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::fs::read_to_string("../../VERSION").expect("read release version");
        let number: u16 = version
            .trim()
            .strip_prefix('v')
            .expect("release label starts with v")
            .parse()
            .expect("numeric release label");
        let numeric_version = u64::from(number) << 16; // 0.0.N.0
        winresource::WindowsResource::new()
            .set_icon("../../packaging/icons/Fountage.ico")
            .set("ProductName", "Fountage by Baz Studios")
            .set("FileDescription", "Fountage writing studio preview")
            .set("ProductVersion", version.trim())
            .set("FileVersion", version.trim())
            .set_version_info(winresource::VersionInfo::FILEVERSION, numeric_version)
            .set_version_info(winresource::VersionInfo::PRODUCTVERSION, numeric_version)
            .compile()
            .expect("compile Windows icon/version resources");
    }
    slint_build::compile_with_config(
        "ui/studio.slint",
        slint_build::CompilerConfiguration::new().with_debug_info(cfg!(debug_assertions)),
    )
    .expect("compile prototype UI");
}
