use super::{framework_resolver, FrameworkResolver};

framework_resolver!(FiberResolver, "fiber", "go");

pub fn resolver() -> impl FrameworkResolver {
    FiberResolver
}
