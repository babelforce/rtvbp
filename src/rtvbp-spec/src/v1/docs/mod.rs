pub(crate) mod asyncapischema;
pub mod generate;
pub(crate) mod provider;

pub const EXAMPLE_RESOURCE_ID: &str = "VPk_6IQStguK0vJrdJ4mT";

pub trait Example {
    fn example() -> Self;
}

pub trait Examples
where
    Self: Sized,
{
    fn examples() -> Vec<Self>;
}

impl<T> Examples for T
where
    T: Example,
{
    fn examples() -> Vec<Self> {
        vec![Self::example()]
    }
}
