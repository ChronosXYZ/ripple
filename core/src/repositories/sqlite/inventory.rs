use crate::network::messages::Object;
use crate::pow;
use std::{
    collections::{hash_map::RandomState, HashSet},
    error::Error,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{QueryBuilder, SqlitePool};

use crate::repositories::inventory::InventoryRepository;

use super::models::{self};

#[derive(Clone)]
pub struct SqliteInventoryRepository {
    pool: SqlitePool,
}

impl SqliteInventoryRepository {
    pub fn new(conn_pool: SqlitePool) -> SqliteInventoryRepository {
        SqliteInventoryRepository { pool: conn_pool }
    }

    fn deserialize_model(m: models::Object) -> Object {
        Object {
            hash: bs58::decode(m.hash).into_vec().unwrap(),
            nonce: if let Some(n) = m.nonce { n } else { vec![] },
            expires: m.expires.timestamp(),
            kind: serde_cbor::from_slice(&m.data).expect("data not to be malformed"),
            signature: m.signature.clone(),
            nonce_trials_per_byte: pow::NETWORK_MIN_NONCE_TRIALS_PER_BYTE, // FIXME save this in db
            extra_bytes: pow::NETWORK_MIN_EXTRA_BYTES,                     // FIXME save this in db
        }
    }
}

#[async_trait]
impl InventoryRepository for SqliteInventoryRepository {
    async fn get(&self) -> Result<Vec<String>, Box<dyn Error>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT hash FROM inventory WHERE expires > ? AND nonce IS NOT NULL",
        )
        .bind(Utc::now())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn get_object(&self, hash: String) -> Result<Option<Object>, Box<dyn Error>> {
        let obj: Result<models::Object, sqlx::Error> =
            sqlx::query_as("SELECT * FROM inventory WHERE hash = ? AND nonce IS NOT NULL")
                .bind(&hash)
                .fetch_one(&self.pool)
                .await;

        if obj.is_err() {
            match obj {
                Err(sqlx::Error::RowNotFound) => return Ok(None),
                _ => return Err(Box::new(obj.err().unwrap())),
            }
        }

        let obj = obj.unwrap();
        Ok(Some(Self::deserialize_model(obj)))
    }

    /// Filter inventory vector with missing objects
    async fn get_missing_objects(
        &self,
        hashes: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let incoming_objects: HashSet<String, RandomState> =
            HashSet::from_iter(hashes.clone().into_iter());

        let existing_objects = self.get().await?;

        let existing_objects = HashSet::from_iter(existing_objects.into_iter());

        let missing_objects: Vec<String> = incoming_objects
            .difference(&existing_objects)
            .map(|x| x.clone())
            .collect();

        Ok(missing_objects)
    }

    /// Store received object
    async fn store_object(&mut self, o: Object) -> Result<(), Box<dyn Error>> {
        let hash = bs58::encode(&o.hash).into_string();
        let data = serde_cbor::to_vec(&o.kind).expect("data not to be malformed");

        let model = models::Object {
            hash,
            nonce: if o.nonce.is_empty() {
                None
            } else {
                Some(o.nonce)
            },
            object_type: o.kind.object_type() as i32,
            data,
            expires: DateTime::from_timestamp(o.expires, 0).unwrap(),
            signature: o.signature,
        };

        QueryBuilder::new(
            "INSERT INTO inventory (hash, nonce, object_type, data, expires, signature) ",
        )
        .push_values([model], |mut b, model| {
            b.push_bind(model.hash)
                .push_bind(model.nonce)
                .push_bind(model.object_type)
                .push_bind(model.data)
                .push_bind(model.expires)
                .push_bind(model.signature);
        })
        .build()
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_missing_pow_objects(&self) -> Result<Vec<Object>, Box<dyn Error>> {
        let res =
            sqlx::query_as::<_, models::Object>("SELECT * FROM inventory WHERE nonce IS NULL")
                .fetch_all(&self.pool)
                .await?;
        let mut objects = vec![];
        res.into_iter().for_each(|m| {
            objects.push(Self::deserialize_model(m));
        });
        Ok(objects)
    }

    async fn update_nonce(&mut self, hash: String, nonce: Vec<u8>) -> Result<(), Box<dyn Error>> {
        sqlx::query("UPDATE inventory SET nonce = ? WHERE hash = ?")
            .bind(nonce)
            .bind(hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Cleanup the storage of expired items
    async fn cleanup(&mut self) -> Result<usize, Box<dyn Error>> {
        let result = sqlx::query("DELETE FROM inventory WHERE expires <= ?")
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() as usize)
    }
}
