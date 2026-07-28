//! End-to-end persistence and CLI coverage for framework semantic facts.

use std::fs;
use std::path::Path;
use std::process::Command;

use graxus_index::sqlite::SqliteStore;
use tempfile::TempDir;

fn bin() -> String {
    env!("CARGO_BIN_EXE_graxus").to_owned()
}

fn run(args: &[&str]) -> String {
    let output = Command::new(bin())
        .args(args)
        .output()
        .expect("failed to execute graxus binary");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "graxus {:?} failed with {:?}\nstdout:\n{}\nstderr:\n{}",
        args,
        output.status,
        stdout,
        stderr
    );
    stdout
}

fn write_aspnet_fixture(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/Api.cs"),
        r#"var builder = WebApplication.CreateBuilder(args);
builder.Services.AddScoped<IUserService, UserService>();
var app = builder.Build();
app.MapGet("/users", GetUsers);
IResult GetUsers() => Results.Ok();
public interface IUserService {}
public class UserService : IUserService {}
"#,
    )
    .unwrap();
}

#[test]
fn indexes_semantic_facts_into_cli_json_and_sqlite() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    let root_str = root.to_str().unwrap();

    run(&["init", root_str]);
    write_aspnet_fixture(root);
    run(&["--root", root_str, "index", "--codemap-backend", "ripex"]);

    let routes = run(&[
        "--root",
        root_str,
        "routes",
        "--framework",
        "aspnet",
        "--json",
    ]);
    assert!(routes.contains("/users"), "routes output:\n{routes}");
    assert!(routes.contains("GetUsers"), "routes output:\n{routes}");

    let types = run(&[
        "--root",
        root_str,
        "types",
        "--name",
        "IUserService",
        "--json",
    ]);
    assert!(
        types.contains("UserService") && types.contains("IUserService"),
        "types output:\n{types}"
    );
    let deadcode: serde_json::Value =
        serde_json::from_str(&run(&["--root", root_str, "dead-code", "--json"])).unwrap();
    assert!(deadcode.is_array(), "deadcode JSON must be an array");

    let impact: serde_json::Value = serde_json::from_str(&run(&[
        "--root",
        root_str,
        "impact",
        "src/Api.cs",
        "--json",
    ]))
    .unwrap();
    assert_eq!(impact["file"], "src/Api.cs");
    assert!(
        impact["target_symbols"]
            .as_array()
            .is_some_and(|symbols| symbols.iter().any(|symbol| symbol == "UserService")),
        "impact output: {impact}"
    );

    let db_path = root.join(".graxus/index.db");
    let db = SqliteStore::new(&db_path).unwrap();
    let stored_routes = db.get_routes_by_path("/users").unwrap();
    assert_eq!(stored_routes.len(), 1);
    assert_eq!(stored_routes[0].framework, "aspnet");
    assert_eq!(stored_routes[0].handler, "GetUsers");

    let stored_types = db.get_type_impls_by_trait("IUserService").unwrap();
    assert_eq!(stored_types.len(), 1);
    assert_eq!(stored_types[0].implementing_type, "UserService");

    let stored_di = db.get_di_bindings_by_abstract_type("IUserService").unwrap();
    assert_eq!(stored_di.len(), 1);
    assert_eq!(stored_di[0].concrete_type, "UserService");
    assert_eq!(stored_di[0].lifetime.as_deref(), Some("scoped"));
}
