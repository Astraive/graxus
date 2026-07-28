use super::{framework_resolver, FrameworkResolver};

framework_resolver!(GinResolver, "gin", "go");

pub fn resolver() -> impl FrameworkResolver {
    GinResolver
}
