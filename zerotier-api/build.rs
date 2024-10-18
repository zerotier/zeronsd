use anyhow::Result;

fn generate(apiname: &str) -> Result<()> {
    let src = format!("specs/{}v1.json", apiname);
    println!("cargo:rerun-if-changed={}", src);
    let file = std::fs::File::open(&src)?;
    let spec = serde_json::from_reader(file)?;
    let mut generator = progenitor::Generator::default();

    let tokens = generator.generate_tokens(&spec).unwrap();
    let ast = syn::parse2(tokens).unwrap();
    let content = prettyplease::unparse(&ast);

    let mut out_file = std::path::Path::new(&std::env::var("OUT_DIR")?).to_path_buf();
    out_file.push(&format!("{}.rs", apiname));

    std::fs::write(out_file, content).unwrap();

    Ok(())
}

fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=build.rs");
    generate("central")?;
    generate("service")?;

    Ok(())
}
