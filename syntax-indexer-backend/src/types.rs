use sqlx::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "Text")]
#[sqlx(rename_all = "snake_case")]
pub enum BondStatus {
    NewlyLaunched,
    Graduating,
    Graduated,
}
