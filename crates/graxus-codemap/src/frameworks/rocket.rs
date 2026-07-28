use super::{framework_resolver, FrameworkResolver};

framework_resolver!(RocketResolver, "rocket", "rust");

pub fn resolver() -> impl FrameworkResolver {
    RocketResolver
}
