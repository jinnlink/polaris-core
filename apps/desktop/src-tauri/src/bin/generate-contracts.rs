fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/contracts/core.ts");
    let generated = polaris_desktop_lib::contracts::generated_typescript_contracts();
    if std::env::args().any(|argument| argument == "--check") {
        let committed = std::fs::read_to_string(&path).expect("read committed TypeScript contract");
        if committed != generated {
            eprintln!("Rust/TypeScript DTO drift: regenerate {}", path.display());
            std::process::exit(1);
        }
        return;
    }
    std::fs::write(&path, generated).expect("write generated TypeScript contract");
    println!("generated {}", path.display());
}
