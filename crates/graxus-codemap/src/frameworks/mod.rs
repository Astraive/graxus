use crate::facts::{DIFact, RouteFact};

pub mod actix;
pub mod aspnet;
pub mod axum;
pub mod crow;
pub mod django;
pub mod drogon;
pub mod echo;
pub mod express;
pub mod fastapi;
pub mod fiber;
pub mod flask;
pub mod gin;
pub mod nestjs;
pub mod nextjs;
pub mod pistache;
pub mod rocket;

#[derive(Debug, Clone, Copy)]
pub struct FrameworkDescriptor {
    pub name: &'static str,
    pub language: &'static str,
}

pub trait FrameworkResolver {
    fn descriptor(&self) -> FrameworkDescriptor;

    fn extract_routes(&self, _file: &str, _source: &str) -> Vec<RouteFact> {
        Vec::new()
    }

    fn extract_di_bindings(&self, _file: &str, _source: &str) -> Vec<DIFact> {
        Vec::new()
    }
}

/// Extract framework-specific HTTP endpoint registrations for one source file.
///
/// Framework parsers deliberately run after the language parser. They add
/// framework semantics—routes and their handlers—that are not represented by
/// the language-neutral Ripex fact model.
pub fn extract_routes(file: &str, source: &str, language: &str) -> Vec<RouteFact> {
    let mut routes = match language {
        "python" => {
            let mut routes = fastapi::resolver().extract_routes(file, source);
            routes.extend(flask::resolver().extract_routes(file, source));
            routes.extend(django::resolver().extract_routes(file, source));
            routes
        }
        "rust" => {
            let mut routes = axum::resolver().extract_routes(file, source);
            routes.extend(actix::resolver().extract_routes(file, source));
            routes.extend(rocket::resolver().extract_routes(file, source));
            routes
        }
        "go" => {
            let mut routes = gin::resolver().extract_routes(file, source);
            routes.extend(fiber::resolver().extract_routes(file, source));
            routes.extend(echo::resolver().extract_routes(file, source));
            routes
        }
        "javascript" | "typescript" => {
            let mut routes = express::resolver().extract_routes(file, source);
            routes.extend(nestjs::resolver().extract_routes(file, source));
            routes.extend(nextjs::resolver().extract_routes(file, source));
            routes
        }
        "csharp" => aspnet::resolver().extract_routes(file, source),
        "cpp" => {
            let mut routes = crow::resolver().extract_routes(file, source);
            routes.extend(pistache::resolver().extract_routes(file, source));
            routes.extend(drogon::resolver().extract_routes(file, source));
            routes
        }
        _ => Vec::new(),
    };

    routes.sort_by(|left, right| {
        (
            left.framework.as_str(),
            left.method.as_str(),
            left.path.as_str(),
            left.handler.as_str(),
            left.line,
        )
            .cmp(&(
                right.framework.as_str(),
                right.method.as_str(),
                right.path.as_str(),
                right.handler.as_str(),
                right.line,
            ))
    });
    routes.dedup_by(|left, right| {
        left.framework == right.framework
            && left.method == right.method
            && left.path == right.path
            && left.handler == right.handler
            && left.line == right.line
    });
    routes
}

pub fn supported_frameworks() -> Vec<FrameworkDescriptor> {
    vec![
        fastapi::resolver().descriptor(),
        django::resolver().descriptor(),
        axum::resolver().descriptor(),
        actix::resolver().descriptor(),
        crow::resolver().descriptor(),
        rocket::resolver().descriptor(),
        flask::resolver().descriptor(),
        gin::resolver().descriptor(),
        fiber::resolver().descriptor(),
        echo::resolver().descriptor(),
        express::resolver().descriptor(),
        nestjs::resolver().descriptor(),
        nextjs::resolver().descriptor(),
        aspnet::resolver().descriptor(),
        drogon::resolver().descriptor(),
        pistache::resolver().descriptor(),
    ]
}
