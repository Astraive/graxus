use super::{framework_resolver, FrameworkResolver};

framework_resolver!(NestJsResolver, "nestjs", "typescript");

pub fn resolver() -> impl FrameworkResolver {
    NestJsResolver
}
