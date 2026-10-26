use std::path::Path;
use std::process::Command;

// {test}`requirements.packaging.binary_only`
#[test]
fn cargo_package_has_binary_and_no_library_target() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata");
    let package = metadata["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|package| package["name"].as_str() == Some("piraeus"))
        .expect("piraeus package");
    let targets = package["targets"].as_array().expect("package targets");

    let mut has_binary = false;
    for target in targets {
        let name = target["name"].as_str().expect("target name");
        for kind in target["kind"].as_array().expect("target kinds") {
            let kind = kind.as_str().expect("target kind");
            has_binary |= name == "piraeus" && kind == "bin";
            assert!(
                !kind.ends_with("lib") && kind != "proc-macro",
                "unexpected library target {name}: {kind}"
            );
        }
    }
    assert!(has_binary, "missing piraeus binary target");
}
