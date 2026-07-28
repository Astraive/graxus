use super::{framework_resolver, FrameworkResolver};

framework_resolver!(EchoResolver, "echo", "go");

pub fn resolver() -> impl FrameworkResolver {
    EchoResolver
}
