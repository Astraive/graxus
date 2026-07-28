use super::{framework_resolver, FrameworkResolver};

framework_resolver!(FastApiResolver, "fastapi", "python");

pub fn resolver() -> impl FrameworkResolver {
    FastApiResolver
}
