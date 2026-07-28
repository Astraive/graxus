use graxus_index::{schema, SqliteStore};
use std::path::PathBuf;

fn temp_db() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "graxus-index-schema-smoke-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("test.db")
}
#[test]
fn schema_includes_new_fact_tables() {
    let ddl = schema::all_statements();
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS routes"));
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS type_impls"));
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS di_bindings"));
}

#[test]
fn semantic_fact_tables_round_trip() {
    let store = SqliteStore::new(&temp_db()).unwrap();
    let middleware = vec!["authenticate".to_owned(), "validate \"id\"".to_owned()];

    store
        .insert_route(
            "route:items:detail",
            "src/routes.rs",
            "rust",
            "GET",
            "/api/items/:id",
            "stale_handler",
            None,
            42,
            "axum",
            &[],
        )
        .unwrap();
    store
        .insert_route(
            "route:items:index",
            "src/routes.rs",
            "rust",
            "GET",
            "/api/items/:id",
            "list_items",
            Some("src/handlers/items.rs"),
            10,
            "axum",
            &["authenticate".to_owned()],
        )
        .unwrap();
    store
        .insert_route(
            "route:items:detail",
            "src/routes.rs",
            "rust",
            "GET",
            "/api/items/:id",
            "get_item",
            Some("src/handlers/items.rs"),
            42,
            "axum",
            &middleware,
        )
        .unwrap();

    let routes = store.get_routes_by_path("/api/items/:id").unwrap();
    assert_eq!(
        routes
            .iter()
            .map(|route| route.id.as_str())
            .collect::<Vec<_>>(),
        vec!["route:items:index", "route:items:detail"]
    );
    let route = &routes[1];
    assert_eq!(route.file, "src/routes.rs");
    assert_eq!(route.language, "rust");
    assert_eq!(route.method, "GET");
    assert_eq!(route.path, "/api/items/:id");
    assert_eq!(route.handler, "get_item");
    assert_eq!(route.handler_file.as_deref(), Some("src/handlers/items.rs"));
    assert_eq!(route.line, 42);
    assert_eq!(route.framework, "axum");
    assert_eq!(route.middleware, middleware);

    store
        .insert_type_impl(
            "type_impl:service",
            "src/services.rs",
            "rust",
            "StaleService",
            "ItemService",
            80,
            "trait_impl",
        )
        .unwrap();
    store
        .insert_type_impl(
            "type_impl:alternate",
            "src/services.rs",
            "rust",
            "AlternateService",
            "ItemService",
            15,
            "trait_impl",
        )
        .unwrap();
    store
        .insert_type_impl(
            "type_impl:service",
            "src/services.rs",
            "rust",
            "PostgresItemService",
            "ItemService",
            80,
            "trait_impl",
        )
        .unwrap();

    let type_impls = store.get_type_impls_by_trait("ItemService").unwrap();
    assert_eq!(
        type_impls
            .iter()
            .map(|type_impl| type_impl.id.as_str())
            .collect::<Vec<_>>(),
        vec!["type_impl:alternate", "type_impl:service"]
    );
    let type_impl = &type_impls[1];
    assert_eq!(type_impl.file, "src/services.rs");
    assert_eq!(type_impl.language, "rust");
    assert_eq!(type_impl.implementing_type, "PostgresItemService");
    assert_eq!(type_impl.trait_or_interface, "ItemService");
    assert_eq!(type_impl.line, 80);
    assert_eq!(type_impl.kind, "trait_impl");

    store
        .insert_di_binding(
            "di:item-service",
            "src/container.rs",
            "rust",
            "ItemService",
            "StaleItemService",
            None,
            70,
            "shaku",
        )
        .unwrap();
    store
        .insert_di_binding(
            "di:item-service:test",
            "src/container.rs",
            "rust",
            "ItemService",
            "TestItemService",
            Some("transient"),
            20,
            "shaku",
        )
        .unwrap();
    store
        .insert_di_binding(
            "di:item-service",
            "src/container.rs",
            "rust",
            "ItemService",
            "PostgresItemService",
            Some("singleton"),
            70,
            "shaku",
        )
        .unwrap();

    let bindings = store
        .get_di_bindings_by_abstract_type("ItemService")
        .unwrap();
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.id.as_str())
            .collect::<Vec<_>>(),
        vec!["di:item-service:test", "di:item-service"]
    );
    let binding = &bindings[1];
    assert_eq!(binding.file, "src/container.rs");
    assert_eq!(binding.language, "rust");
    assert_eq!(binding.abstract_type, "ItemService");
    assert_eq!(binding.concrete_type, "PostgresItemService");
    assert_eq!(binding.lifetime.as_deref(), Some("singleton"));
    assert_eq!(binding.line, 70);
    assert_eq!(binding.framework, "shaku");
}
