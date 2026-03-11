#[cfg(target_os = "windows")]
fn main() {
    println!("cargo:rerun-if-changed=assets/icon/icon.ico");

    if let Err(error) = winresource::WindowsResource::new()
        .set_icon("assets/icon/icon.ico")
        .compile()
    {
        panic!("failed to compile Windows resources: {error}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}
