use super::{framework_resolver, FrameworkResolver};

framework_resolver!(DjangoResolver, "django", "python");

pub fn resolver() -> impl FrameworkResolver {
    DjangoResolver
}
