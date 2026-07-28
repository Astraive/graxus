use super::{framework_resolver, FrameworkResolver};

framework_resolver!(CrowResolver, "crow", "cpp");

pub fn resolver() -> impl FrameworkResolver {
    CrowResolver
}
