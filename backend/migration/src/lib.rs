pub use sea_orm_migration::prelude::*;

mod m20240327_000001_create_users;
mod m20240327_000002_create_accounts;
mod m20240327_000003_create_api_keys;
mod m20240327_000004_create_usages;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240327_000001_create_users::Migration),
            Box::new(m20240327_000002_create_accounts::Migration),
            Box::new(m20240327_000003_create_api_keys::Migration),
            Box::new(m20240327_000004_create_usages::Migration),
        ]
    }
}
