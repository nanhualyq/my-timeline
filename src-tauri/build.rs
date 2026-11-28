fn main() {
    // tauri-build does not declare the icon files as build inputs, so the
    // compiled Windows resource (target/*/build/my-timeline-*/out/resource.lib)
    // keeps whatever icon it had the first time it was generated. Without
    // these lines a regenerated icon.ico never reaches the .exe, and every
    // rebuild silently relinks against the stale resource.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");

    tauri_build::build()
}
