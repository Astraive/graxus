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

macro_rules! framework_resolver {
    ($type_name:ident, $display_name:literal, $language:literal) => {
        #[derive(Debug, Default, Clone, Copy)]
        pub struct $type_name;

        impl FrameworkResolver for $type_name {
            fn descriptor(&self) -> $crate::frameworks::FrameworkDescriptor {
                $crate::frameworks::FrameworkDescriptor {
                    name: $display_name,
                    language: $language,
                }
            }
        }
    };
}

pub(crate) use framework_resolver;

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
