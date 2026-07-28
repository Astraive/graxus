use super::{framework_resolver, FrameworkResolver};

framework_resolver!(PistacheResolver, "pistache", "cpp");

pub fn resolver() -> impl FrameworkResolver {
    PistacheResolver
}
