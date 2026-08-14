include!("zz_generated_types.rs");
include!("zz_generated_roles.rs");

#[cfg(test)]
mod generated_golden_tests {
    include!("zz_generated_golden_tests.rs");
}

#[cfg(test)]
mod generated_roles_tests {
    include!("zz_generated_roles_tests.rs");
}
