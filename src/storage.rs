use crate::{crypto::Encryption, error};

type CustomResult<T, C> = error_stack::Result<T, C>;
use std::sync::Arc;

use diesel_async::{
    pooled_connection::{
        self,
        deadpool::{Object, Pool},
    },
    AsyncPgConnection,
};
use error_stack::ResultExt;
use masking::Secret;

pub mod db;
pub mod schema;
pub mod types;

pub trait State {}

/// Storage State that is to be passed though the application
#[derive(Clone)]
pub struct Storage {
    pg_pool: Arc<Pool<AsyncPgConnection>>,
}

type DeadPoolConnType = Object<AsyncPgConnection>;

impl Storage {
    /// Create a new storage interface from configuration
    pub async fn new(database_url: String) -> error_stack::Result<Self, error::StorageError> {
        let config =
            pooled_connection::AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
        let pool = Pool::builder(config)
            .build()
            .change_context(error::StorageError::DBPoolError)?;
        Ok(Self {
            pg_pool: Arc::new(pool),
        })
    }

    /// Get connection from database pool for accessing data
    pub async fn get_conn(&self) -> error_stack::Result<DeadPoolConnType, error::StorageError> {
        self.pg_pool
            .get()
            .await
            .change_context(error::StorageError::PoolClientFailure)
    }
}

/// 
/// MerchantInterface:
/// 
/// Interface providing functional to interface with the merchant table in database
#[async_trait::async_trait]
pub trait MerchantInterface {
    type Algorithm: Encryption<Vec<u8>, Vec<u8>>;

    /// find merchant from merchant table with `merchant_id` and `tenant_id` with key as master key
    async fn find_by_merchant_id(
        &self,
        merchant_id: String,
        tenant_id: String,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Merchant, error::StorageError>;

    /// find merchant from merchant table with `merchant_id` and `tenant_id` with key as master key
    /// and if not found create a new merchant
    async fn find_or_create_by_merchant_id(
        &self,
        merchant_id: String,
        tenant_id: String,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Merchant, error::StorageError>;

    /// Insert a new merchant in the database by encrypting the dek with `master_key`
    async fn insert_merchant(
        &self,
        new: types::MerchantNew,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Merchant, error::StorageError>;
}


///
/// LockerInterface:
///
/// Interface for interacting with the locker database table
#[async_trait::async_trait]
pub trait LockerInterface {
    type Algorithm: Encryption<Vec<u8>, Vec<u8>>;
    /// Fetch payment data from locker table by decrypting with `dek`
    async fn find_by_locker_id_merchant_id_customer_id(
        &self,
        locker_id: Secret<String>,
        tenant_id: String,
        merchant_id: String,
        customer_id: String,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Locker, error::StorageError>;


    /// Insert payment data from locker table by decrypting with `dek`
    async fn insert_or_get_from_locker(
        &self,
        new: types::LockerNew,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Locker, error::StorageError>;

    /// Delete card from the locker, without access to the `dek`
    async fn delete_from_locker(
        &self,
        locker_id: Secret<String>,
        tenant_id: String,
        merchant_id: String,
        customer_id: String,
    ) -> CustomResult<usize, error::StorageError>;
}