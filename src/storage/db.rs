use diesel::BoolExpressionMethods;
use diesel::{associations::HasTable, ExpressionMethods, QueryDsl};
use diesel_async::RunQueryDsl;
use error_stack::ResultExt;
use masking::ExposeInterface;
use masking::Secret;

use crate::crypto::aes::{generate_aes256_key, GcmAes256};
use crate::error;

use super::types::StorageDecryption;
use super::types::StorageEncryption;
use super::{schema, types, CustomResult, LockerInterface, MerchantInterface, Storage};

#[async_trait::async_trait]
impl MerchantInterface for Storage {
    type Algorithm = GcmAes256;

    async fn find_by_merchant_id(
        &self,
        merchant_id: String,
        tenant_id: String,
        key: &GcmAes256,
    ) -> CustomResult<types::Merchant, error::StorageError> {
        let mut conn = self.get_conn().await?;
        let output: Result<types::MerchantInner, diesel::result::Error> =
            types::MerchantInner::table()
                .filter(
                    schema::merchant::merchant_id
                        .eq(merchant_id.clone())
                        .and(schema::merchant::tenant_id.eq(tenant_id.clone())),
                )
                .get_result(&mut conn)
                .await;
        output
            .change_context(error::StorageError::FindError)
            .and_then(|inner| {
                inner
                    .decrypt(key)
                    .change_context(error::StorageError::DecryptionError)
            })
    }

    async fn find_or_create_by_merchant_id(
        &self,
        merchant_id: String,
        tenant_id: String,
        key: &GcmAes256,
    ) -> CustomResult<types::Merchant, error::StorageError> {
        let mut conn = self.get_conn().await?;

        let output: Result<types::MerchantInner, diesel::result::Error> =
            types::MerchantInner::table()
                .filter(
                    schema::merchant::merchant_id
                        .eq(merchant_id.clone())
                        .and(schema::merchant::tenant_id.eq(tenant_id.clone())),
                )
                .get_result(&mut conn)
                .await;
        match output {
            Ok(inner) => inner
                .decrypt(key)
                .change_context(error::StorageError::DecryptionError),
            Err(inner_err) => match inner_err {
                diesel::result::Error::NotFound => {
                    self.insert_merchant(
                        types::MerchantNew {
                            merchant_id,
                            tenant_id,
                            enc_key: generate_aes256_key().to_vec().into(),
                        },
                        key,
                    )
                    .await
                }
                output => Err(output).change_context(error::StorageError::FindError),
            },
        }
    }
    async fn insert_merchant(
        &self,
        new: types::MerchantNew,
        key: &GcmAes256,
    ) -> CustomResult<types::Merchant, error::StorageError> {
        let mut conn = self.get_conn().await?;
        let query = diesel::insert_into(types::MerchantInner::table()).values(
            new.encrypt(key)
                .change_context(error::StorageError::FindError)?,
        );

        query
            .get_result(&mut conn)
            .await
            .map_err(error_stack::Report::from)
            .change_context(error::StorageError::FindError)
            .and_then(|inner: types::MerchantInner| {
                inner
                    .decrypt(key)
                    .change_context(error::StorageError::DecryptionError)
            })
    }
}

#[async_trait::async_trait]
impl LockerInterface for Storage {
    type Algorithm = GcmAes256;
    async fn find_by_locker_id_merchant_id_customer_id(
        &self,
        locker_id: Secret<String>,
        tenant_id: String,
        merchant_id: String,
        customer_id: String,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Locker, error::StorageError> {
        let mut conn = self.get_conn().await?;

        types::LockerInner::table()
            .filter(
                schema::locker::locker_id
                    .eq(locker_id.expose())
                    .and(schema::locker::tenant_id.eq(tenant_id))
                    .and(schema::locker::merchant_id.eq(merchant_id))
                    .and(schema::locker::customer_id.eq(customer_id)),
            )
            .get_result(&mut conn)
            .await
            .map_err(error_stack::Report::from)
            .change_context(error::StorageError::FindError)
            .and_then(|inner: types::LockerInner| {
                inner
                    .decrypt(key)
                    .change_context(error::StorageError::DecryptionError)
            })
    }
    async fn insert_or_get_from_locker(
        &self,
        new: types::LockerNew,
        key: &Self::Algorithm,
    ) -> CustomResult<types::Locker, error::StorageError> {
        let mut conn = self.get_conn().await?;
        let cloned_new = new.clone();

        let query: Result<_, diesel::result::Error> =
            diesel::insert_into(types::LockerInner::table())
                .values(
                    new.encrypt(key)
                        .change_context(error::StorageError::EncryptionError)?,
                )
                .get_result::<types::LockerInner>(&mut conn)
                .await;

        match query {
            Ok(inner) => inner
                .decrypt(key)
                .change_context(error::StorageError::DecryptionError),
            Err(error) => match error {
                diesel::result::Error::DatabaseError(
                    diesel::result::DatabaseErrorKind::UniqueViolation,
                    _,
                ) => {
                    self.find_by_locker_id_merchant_id_customer_id(
                        cloned_new.locker_id,
                        cloned_new.tenant_id,
                        cloned_new.merchant_id,
                        cloned_new.customer_id,
                        key,
                    )
                    .await
                }
                error => Err(error).change_context(error::StorageError::FindError),
            },
        }
    }

    async fn delete_from_locker(
        &self,
        locker_id: Secret<String>,
        tenant_id: String,
        merchant_id: String,
        customer_id: String,
    ) -> CustomResult<usize, error::StorageError> {
        let mut conn = self.get_conn().await?;

        let query = diesel::delete(types::LockerInner::table()).filter(
            schema::locker::locker_id
                .eq(locker_id.expose())
                .and(schema::locker::tenant_id.eq(tenant_id))
                .and(schema::locker::merchant_id.eq(merchant_id))
                .and(schema::locker::customer_id.eq(customer_id)),
        );

        query
            .execute(&mut conn)
            .await
            .map_err(error_stack::Report::from)
            .change_context(error::StorageError::FindError)
    }
}
