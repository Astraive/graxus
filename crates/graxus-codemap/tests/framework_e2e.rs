//! End-to-end framework intelligence coverage.
//!
//! This test creates one repository containing representative source for every
//! currently supported framework family. It exercises the full CodemapBuilder
//! pipeline: Ripex-first language extraction, framework route recognition,
//! handler resolution, type relationships, and dependency-injection facts.

use graxus_codemap::CodemapBuilder;
use graxus_core::{FileKind, Language, ParserBackend, ScannedFile};

fn scanned(path: std::path::PathBuf, relative_path: &str, language: Language) -> ScannedFile {
    let size = std::fs::metadata(&path).unwrap().len();
    ScannedFile {
        path,
        relative_path: relative_path.to_owned(),
        kind: FileKind::Code,
        language,
        hash: "framework-e2e".to_owned(),
        size,
        modified: chrono::Utc::now(),
    }
}

#[test]
fn builds_framework_semantics_from_a_multilanguage_repository() {
    let root = std::env::temp_dir().join(format!(
        "graxus-framework-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();

    let python = root.join("api.py");
    std::fs::write(
        &python,
        r#"from fastapi import FastAPI
app = FastAPI()

@app.get("/users")
async def list_users():
    return []
"#,
    )
    .unwrap();

    let rust = root.join("api.rs");
    std::fs::write(
        &rust,
        r#"use axum::{routing::get, Router};
trait Repository {}
struct Users;
impl Repository for Users {}
async fn list_users() {}
fn router() { let _app = Router::new().route("/users", get(list_users)); }
"#,
    )
    .unwrap();

    let go = root.join("api.go");
    std::fs::write(
        &go,
        r#"package api
import "github.com/gin-gonic/gin"
func listUsers(c *gin.Context) {}
func Register(r *gin.Engine) { r.GET("/users", listUsers) }
"#,
    )
    .unwrap();

    let typescript = root.join("api.ts");
    std::fs::write(
        &typescript,
        r#"import express from "express";
import { Injectable } from "@nestjs/common";
interface UserRepository {}
@Injectable()
class Users implements UserRepository {}
const app = express();
function listUsers() { return []; }
app.get("/users", listUsers);
"#,
    )
    .unwrap();

    let csharp = root.join("Api.cs");
    std::fs::write(
        &csharp,
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

    let cpp = root.join("api.cpp");
    std::fs::write(
        &cpp,
        r#"#include <crow.h>
crow::SimpleApp app;
void users() {}
void register_routes() { CROW_ROUTE(app, "/users")(users); }
"#,
    )
    .unwrap();

    let graph = CodemapBuilder::new(vec![
        scanned(python, "api.py", Language::Python),
        scanned(rust, "api.rs", Language::Rust),
        scanned(go, "api.go", Language::Go),
        scanned(typescript, "api.ts", Language::TypeScript),
        scanned(csharp, "Api.cs", Language::CSharp),
        scanned(cpp, "api.cpp", Language::Cpp),
    ])
    .with_backend(ParserBackend::Ripex)
    .build()
    .unwrap();

    let expected_frameworks = ["fastapi", "axum", "gin", "express", "aspnet", "crow"];
    for framework in expected_frameworks {
        assert!(
            graph
                .routes
                .iter()
                .any(|route| route.framework == framework && route.path == "/users"),
            "missing /users route for {framework}: {:#?}",
            graph.routes
        );
    }
    assert!(graph
        .routes
        .iter()
        .all(|route| !route.id.is_empty() && route.method == route.method.to_ascii_uppercase()));
    assert!(graph
        .routes
        .iter()
        .any(|route| route.handler == "list_users"
            && route.handler_file.as_deref() == Some("api.py")));

    assert!(graph.type_impls.iter().any(|fact| {
        fact.implementing_type == "Users" && fact.trait_or_interface == "Repository"
    }));
    assert!(graph.type_impls.iter().any(|fact| {
        fact.implementing_type == "UserService" && fact.trait_or_interface == "IUserService"
    }));
    assert!(graph.di_bindings.iter().any(|fact| {
        fact.abstract_type == "IUserService"
            && fact.concrete_type == "UserService"
            && fact.lifetime.as_deref() == Some("scoped")
    }));

    std::fs::remove_dir_all(root).unwrap();
}
