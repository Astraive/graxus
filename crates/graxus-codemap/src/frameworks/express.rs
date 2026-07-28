use super::{framework_resolver, FrameworkResolver};

framework_resolver!(ExpressResolver, "express", "javascript");

pub fn resolver() -> impl FrameworkResolver {
    ExpressResolver
}
