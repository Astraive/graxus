use super::{framework_resolver, FrameworkResolver};

framework_resolver!(ActixResolver, "actix", "rust");

pub fn resolver() -> impl FrameworkResolver {
    ActixResolver
}
