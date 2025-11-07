use std::error::Error;

use async_trait::async_trait;
use ecies::{PublicKey, SecretKey};
use sqlx::{QueryBuilder, SqlitePool};

use crate::{network::address::Address, repositories::address::AddressRepository};

use super::models;

#[derive(Clone)]
pub struct SqliteAddressRepository {
    pool: SqlitePool,
}

impl SqliteAddressRepository {
    pub fn new(pool: SqlitePool) -> SqliteAddressRepository {
        SqliteAddressRepository { pool }
    }

    fn serialize(a: Address) -> models::Address {
        let mut pub_sig_key_ser: Option<Vec<u8>> = None;
        let mut priv_sig_key_ser: Option<Vec<u8>> = None;
        let mut pub_enc_key_ser: Option<Vec<u8>> = None;
        let mut priv_enc_key_ser: Option<Vec<u8>> = None;
        if let Some(k) = a.public_signing_key {
            pub_sig_key_ser = Some(k.serialize().to_vec());
        }
        if let Some(k) = a.private_signing_key {
            priv_sig_key_ser = Some(k.serialize().to_vec());
        }
        if let Some(k) = a.public_encryption_key {
            pub_enc_key_ser = Some(k.serialize().to_vec());
        }
        if let Some(k) = a.private_encryption_key {
            priv_enc_key_ser = Some(k.serialize().to_vec());
        }
        models::Address {
            address: a.string_repr,
            tag: bs58::encode(a.tag).into_string(),
            public_signing_key: pub_sig_key_ser,
            private_signing_key: priv_sig_key_ser,
            public_encryption_key: pub_enc_key_ser,
            private_encryption_key: priv_enc_key_ser,
            label: if a.label.is_empty() {
                None
            } else {
                Some(a.label)
            },
        }
    }

    fn deserialize(m: &models::Address) -> anyhow::Result<Address> {
        let mut address = Address::with_string_repr(m.address.clone())?;
        let mut psk = None;
        let mut ppsk = None;
        let mut pek = None;
        let mut ppek = None;
        if let Some(d) = m.public_signing_key.clone() {
            psk = Some(PublicKey::parse_slice(d.as_slice(), None)?);
        }
        if let Some(d) = m.private_signing_key.clone() {
            ppsk = Some(SecretKey::parse_slice(d.as_slice())?);
        }
        if let Some(d) = m.public_encryption_key.clone() {
            pek = Some(PublicKey::parse_slice(d.as_slice(), None)?);
        }
        if let Some(d) = m.private_encryption_key.clone() {
            ppek = Some(SecretKey::parse_slice(d.as_slice())?);
        }

        address.public_signing_key = psk;
        address.private_signing_key = ppsk;
        address.public_encryption_key = pek;
        address.private_encryption_key = ppek;
        address.label = m.label.clone().unwrap_or("".to_string());
        Ok(address)
    }
}

#[async_trait]
impl AddressRepository for SqliteAddressRepository {
    async fn store(&mut self, a: Address) -> Result<(), Box<dyn Error>> {
        let model = Self::serialize(a);
        QueryBuilder::new(
            "INSERT INTO addresses (address, tag, public_encryption_key, public_signing_key, private_signing_key, private_encryption_key, label) "
        )
        .push_values([model], |mut b, model| {
            b.push_bind(model.address)
             .push_bind(model.tag)
             .push_bind(model.public_encryption_key)
             .push_bind(model.public_signing_key)
             .push_bind(model.private_signing_key)
             .push_bind(model.private_encryption_key)
             .push_bind(model.label);
        }).build()
          .execute(&self.pool)
          .await?;
        Ok(())
    }

    async fn delete_address(&mut self, hash: String) -> Result<(), Box<dyn Error>> {
        sqlx::query("DELETE FROM addresses WHERE address = ?")
            .bind(hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_by_ripe_or_tag(&self, hash: String) -> Result<Option<Address>, Box<dyn Error>> {
        let results: Vec<models::Address> =
            sqlx::query_as("SELECT * FROM addresses WHERE address = ? OR tag = ?")
                .bind(&hash)
                .bind(&hash)
                .fetch_all(&self.pool)
                .await?;
        if results.len() == 0 {
            return Ok(None);
        }

        let res: &models::Address = &results[0];
        let address = Self::deserialize(res)?;
        Ok(Some(address))
    }

    async fn get_contacts(&self) -> Result<Vec<Address>, Box<dyn Error>> {
        let results: Vec<models::Address> = sqlx::query_as("SELECT * FROM addresses WHERE public_signing_key IS NOT NULL AND public_encryption_key IS NOT NULL")
            .fetch_all(&self.pool)
            .await?;
        let mut contacts = vec![];
        for res in results {
            let addr = Self::deserialize(&res)?;
            contacts.push(addr);
        }
        Ok(contacts)
    }

    async fn get_identities(&self) -> Result<Vec<Address>, Box<dyn Error>> {
        let results: Vec<models::Address> = sqlx::query_as("SELECT * FROM addresses WHERE private_signing_key IS NOT NULL AND private_encryption_key IS NOT NULL")
            .fetch_all(&self.pool)
            .await?;
        let mut identities = vec![];
        for res in results {
            let addr = Self::deserialize(&res)?;
            identities.push(addr);
        }
        Ok(identities)
    }

    async fn update_public_keys(
        &mut self,
        hash: String,
        public_signing_key: PublicKey,
        public_encryption_key: PublicKey,
    ) -> Result<(), Box<dyn Error>> {
        sqlx::query("UPDATE addresses SET public_signing_key = ?, public_encryption_key = ? WHERE address = ? OR tag = ?")
            .bind(Some(public_signing_key.serialize().to_vec()))
            .bind(Some(public_encryption_key.serialize().to_vec()))
            .bind(&hash)
            .bind(&hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_label(
        &mut self,
        ripe: String,
        new_label: String,
    ) -> Result<(), Box<dyn Error>> {
        sqlx::query("UPDATE addresses SET label = ? WHERE address = ?")
            .bind(Some(new_label))
            .bind(Some(ripe))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
