fn main() {
    println!("cargo:rerun-if-changed=../../VERSION");
    println!("cargo:rerun-if-changed=../../packaging/icons/Fountage.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::fs::read_to_string("../../VERSION").expect("read release version");
        winresource::WindowsResource::new()
            .set_icon("../../packaging/icons/Fountage.ico")
            .set("ProductName", "Fountage by Baz Studios")
            .set("FileDescription", "Fountage writing studio preview")
            .set("ProductVersion", version.trim())
            .set("FileVersion", version.trim())
            .compile()
            .expect("compile Windows icon/version resources");
    }
    slint_build::compile_with_config(
        "ui/studio.slint",
        slint_build::CompilerConfiguration::new().with_debug_info(cfg!(debug_assertions)),
    )
    .expect("compile prototype UI");
}
