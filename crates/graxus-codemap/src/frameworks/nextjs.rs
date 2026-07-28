use super::{framework_resolver, FrameworkResolver};

framework_resolver!(NextJsResolver, "nextjs", "typescript");

pub fn resolver() -> impl FrameworkResolver {
    NextJsResolver
}
