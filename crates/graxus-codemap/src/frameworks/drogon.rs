use super::{framework_resolver, FrameworkResolver};

framework_resolver!(DrogonResolver, "drogon", "cpp");

pub fn resolver() -> impl FrameworkResolver {
    DrogonResolver
}
