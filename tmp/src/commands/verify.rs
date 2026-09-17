use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tmp_core::config::default_config_path;
use tmp_core::evidence::{apply_evidence, HelpEvidenceCheck};
use tmp_core::schema::Schema;
use tmp_core::versioning::save_schema_for_config;

pub fn run(schema_name: &str, config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let config_file_path = match config_path {
        Some(p) => PathBuf::from(p),
        None => default_config_path().ok_or("Could not determine config directory")?,
    };
    let schemas_dir = config_file_path
        .parent()
        .ok_or("Invalid config path")?
        .join("schemas");
    let schema_path = schemas_dir.join(format!("{}.json", schema_name));

    if !schema_path.exists() {
        return Err(format!(
            "Schema '{}' not found at {}",
            schema_name,
            schema_path.display()
        )
        .into());
    }

    let content = std::fs::read_to_string(&schema_path)?;
    let mut schema = Schema::from_json(&content).map_err(|e| format!("Invalid schema: {}", e))?;

    let warnings = schema.validate_strict()?;
    apply_evidence(&mut schema, &HelpEvidenceCheck);
    schema.meta.verified_at = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string(),
    );

    let config_ref: Option<&Path> = config_path.map(Path::new);
    save_schema_for_config(&schema, config_ref)
        .map_err(|e| format!("Failed to save schema: {e}"))?;

    println!("Schema '{}' loaded successfully.", schema_name);
    println!("  Tool: {}", schema.meta.tool);
    println!("  Version: {}", schema.meta.version);
    println!("  Operations: {}", schema.operations.len());
    println!("  Verified: {}", schema.meta.verified);

    for op in &schema.operations {
        println!(
            "  - {} [surface={:?}, effect={:?}, risk={:?}, approval={:?}]",
            op.command, op.surface, op.effect, op.risk, op.approval
        );
        if !op.evidence.is_empty() {
            println!("    evidence: {:?}", op.evidence);
        }
    }

    if warnings.is_empty() {
        println!("\n✓ No warnings.");
    } else {
        println!("\n⚠ {} warning(s):", warnings.len());
        for w in &warnings {
            println!("  - {}", w);
        }
    }

    Ok(())
}
