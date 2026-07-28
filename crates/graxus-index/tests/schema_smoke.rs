use graxus_index::schema;

#[test]
fn schema_includes_new_fact_tables() {
    let ddl = schema::all_statements();
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS routes"));
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS type_impls"));
    assert!(ddl.contains("CREATE TABLE IF NOT EXISTS di_bindings"));
}
