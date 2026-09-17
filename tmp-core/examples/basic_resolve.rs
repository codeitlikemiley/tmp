use tmp_core::context::Context;
use tmp_core::resolve::heuristic_resolve;
use tmp_core::schema::Schema;

fn main() {
    let schema_json = r#"{
        "meta": {
            "tool": "cargo",
            "version": 1,
            "verified": true,
            "keywords": ["rust", "test"]
        },
        "operations": [
            {
                "command": "cargo test",
                "description": "run unit tests",
                "group": "test",
                "parameters": [],
                "effect": "build-test",
                "risk": "low",
                "approval": "not_required"
            }
        ]
    }"#;

    let schema = Schema::from_json(schema_json).expect("valid example schema");
    let cwd = std::env::current_dir().expect("cwd");
    let context = Context::detect(&cwd, None, None);
    match heuristic_resolve("run unit tests", &[schema], &context, None) {
        Some(result) => {
            println!("{}", result.command);
            if let Some(gate) = result.operation {
                println!(
                    "risk={:?} effect={:?} approval={:?}",
                    gate.risk, gate.effect, gate.approval
                );
            }
        }
        None => {
            eprintln!("no operation matched");
            std::process::exit(1);
        }
    }
}
