//! Embeds the package version and description into the Windows executable's
//! version resource, so Explorer's Properties → Details tab shows them.
//! Linux and macOS binaries carry no such metadata; they answer `--version`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // winresource fills FileVersion, ProductVersion, and OriginalFilename from
    // Cargo.toml. The rest it either cannot infer or guesses from the crate name.
    winresource::WindowsResource::new()
        .set("ProductName", "Net Monitor")
        .set("FileDescription", env!("CARGO_PKG_DESCRIPTION"))
        .set("CompanyName", "Casey McCarthy")
        .set("LegalCopyright", "MIT OR Apache-2.0")
        .compile()
        .expect("failed to compile the Windows version resource");

    // On the GNU toolchain (the one the release workflow cross-compiles with)
    // winresource emits `rustc-link-lib`, which cargo passes only to the
    // library target. This binary does not link the library, so hand the
    // compiled resource object to the binary's linker directly.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
        println!("cargo:rustc-link-arg-bins={out_dir}/resource.o");
    }
}
