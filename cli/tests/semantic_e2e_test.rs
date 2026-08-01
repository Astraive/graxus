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

#[test]
fn context_query_is_bounded_and_exposes_semantic_facts() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    let root_str = root.to_str().unwrap();

    run(&["init", root_str]);
    write_aspnet_fixture(root);
    run(&["--root", root_str, "index", "--codemap-backend", "ripex"]);

    let full = run(&[
        "--root", root_str, "context", "--query", "src/Api", "--budget", "10000",
    ]);
    assert!(
        full.contains("Route:") && full.contains("/users"),
        "query:\n{full}"
    );
    assert!(
        full.contains("Type:") && full.contains("UserService"),
        "query:\n{full}"
    );
    assert!(
        full.contains("DI:") && full.contains("IUserService"),
        "query:\n{full}"
    );

    let bounded = run(&[
        "--root",
        root_str,
        "context",
        "--query",
        "src/Api",
        "--budget",
        "40",
        "--max-files",
        "1",
        "--max-symbols",
        "1",
        "--max-notes",
        "1",
        "--depth",
        "0",
        "--min-confidence",
        "100",
    ]);
    assert!(
        bounded.lines().count() < full.lines().count(),
        "tiny query should be bounded:\n{bounded}"
    );
    assert!(
        bounded.contains("Route:") || bounded.contains("Type:") || bounded.contains("DI:"),
        "bounded query should retain semantic facts:\n{bounded}"
    );
    assert!(
        !bounded.contains("AddScoped<IUserService, UserService>"),
        "query must not dump raw source:\n{bounded}"
    );
}

#[test]
fn incremental_and_full_updates_replace_semantic_facts() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    let root_str = root.to_str().unwrap();
    let source = root.join("src/Api.cs");

    run(&["init", root_str]);
    write_aspnet_fixture(root);
    run(&["--root", root_str, "index", "--codemap-backend", "ripex"]);

    fs::write(
        &source,
        r#"var builder = WebApplication.CreateBuilder(args);
builder.Services.AddScoped<IOrderService, OrderService>();
var app = builder.Build();
app.MapGet("/orders", GetOrders);
IResult GetOrders() => Results.Ok();
public interface IOrderService {}
public class OrderService : IOrderService {}
"#,
    )
    .unwrap();
    run(&["--root", root_str, "update", "--codemap-backend", "ripex"]);

    let db_path = root.join(".graxus/index.db");
    let db = SqliteStore::new(&db_path).unwrap();
    assert!(db.get_routes_by_path("/users").unwrap().is_empty());
    assert!(db
        .get_type_impls_by_trait("IUserService")
        .unwrap()
        .is_empty());
    assert!(db
        .get_di_bindings_by_abstract_type("IUserService")
        .unwrap()
        .is_empty());
    assert_eq!(db.get_routes_by_path("/orders").unwrap().len(), 1);
    assert_eq!(
        db.get_type_impls_by_trait("IOrderService").unwrap().len(),
        1
    );
    assert_eq!(
        db.get_di_bindings_by_abstract_type("IOrderService")
            .unwrap()
            .len(),
        1
    );
    drop(db);

    fs::write(&source, "public class PlainService {}\n").unwrap();
    run(&[
        "--root",
        root_str,
        "update",
        "--full",
        "--codemap-backend",
        "ripex",
    ]);

    let db = SqliteStore::new(&db_path).unwrap();
    assert!(db.get_routes_by_path("/orders").unwrap().is_empty());
    assert!(db
        .get_type_impls_by_trait("IOrderService")
        .unwrap()
        .is_empty());
    assert!(db
        .get_di_bindings_by_abstract_type("IOrderService")
        .unwrap()
        .is_empty());
}

#[test]
fn agent_export_json_budget_is_rich_and_bounded() {
    let project = TempDir::new().unwrap();
    let root = project.path();
    let root_str = root.to_str().unwrap();

    run(&["init", root_str]);
    write_aspnet_fixture(root);
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/README.md"),
        "# API\n\nSee [[Usage]] for the endpoint contract.\n",
    )
    .unwrap();
    run(&["--root", root_str, "index", "--codemap-backend", "ripex"]);

    let full: serde_json::Value =
        serde_json::from_str(&run(&["--root", root_str, "agent-export", "--json"]))
            .expect("unbounded agent export must be valid JSON");
    let budget = 1_200usize;
    let raw = run(&[
        "--root",
        root_str,
        "agent-export",
        "--json",
        "--budget",
        &budget.to_string(),
    ]);
    let bounded: serde_json::Value =
        serde_json::from_str(&raw).expect("bounded agent export must be valid JSON");

    assert!(
        bounded.get("project_name").is_some()
            && bounded.get("doc_graph").is_some()
            && bounded.get("code_graph").is_some(),
        "bounded export must use the rich AgentExport shape: {bounded}"
    );
    let full_graph = &full["code_graph"];
    let bounded_graph = &bounded["code_graph"];
    for collection in [
        "files",
        "symbols",
        "imports",
        "calls",
        "routes",
        "type_impls",
        "di_bindings",
        "edges",
        "parser_results",
    ] {
        let full_len = full_graph[collection]
            .as_array()
            .unwrap_or_else(|| panic!("full export missing code_graph.{collection}"))
            .len();
        let bounded_len = bounded_graph[collection]
            .as_array()
            .unwrap_or_else(|| panic!("bounded export missing code_graph.{collection}"))
            .len();
        assert!(
            bounded_len <= full_len,
            "bounded {collection} grew from {full_len} to {bounded_len}"
        );
    }
    for collection in ["nodes", "edges"] {
        let full_len = full["doc_graph"][collection]
            .as_array()
            .unwrap_or_else(|| panic!("full export missing doc_graph.{collection}"))
            .len();
        let bounded_len = bounded["doc_graph"][collection]
            .as_array()
            .unwrap_or_else(|| panic!("bounded export missing doc_graph.{collection}"))
            .len();
        assert!(bounded_len <= full_len);
    }
    assert!(
        bounded_graph["routes"]
            .as_array()
            .is_some_and(|routes| routes.iter().any(|route| route["path"] == "/users")),
        "bounded export lost route semantic fact: {bounded}"
    );
    assert!(
        bounded_graph["type_impls"]
            .as_array()
            .is_some_and(|impls| impls
                .iter()
                .any(|fact| fact["implementing_type"] == "UserService")),
        "bounded export lost type implementation fact: {bounded}"
    );
    assert!(
        bounded_graph["di_bindings"]
            .as_array()
            .is_some_and(|bindings| bindings
                .iter()
                .any(|fact| fact["concrete_type"] == "UserService")),
        "bounded export lost DI fact: {bounded}"
    );
    assert!(
        graxus_agent_api::estimate_tokens(&raw) <= budget,
        "bounded export exceeded {budget} tokens: {}",
        graxus_agent_api::estimate_tokens(&raw)
    );
}
