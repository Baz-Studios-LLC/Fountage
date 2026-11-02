//! Read-only compatibility probe; prints structural facts, never manuscript contents.
use fountage_core::project::Project;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("Usage: cargo run --example inspect_project -- PROJECT.fountage")?;
    let project = Project::read(std::path::Path::new(&path))?;
    println!(
        "{}",
        serde_json::json!({"schema":project.schema(),"format":format!("{:?}",project.format()),"preset":project.preset(),"sourceBytes":project.source().len(),"topLevelEntries":project.entries().len()})
    );
    Ok(())
}
