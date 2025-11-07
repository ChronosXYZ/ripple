use ecies::{PublicKey, SecretKey};
use rand::rngs::OsRng;
use ripemd::{Digest, Ripemd160};
use sha2::Sha512;

#[derive(Clone, Debug)]
pub struct Address {
    pub label: String,
    pub ripe: Vec<u8>,
    pub string_repr: String,
    pub tag: Vec<u8>,
    pub public_decryption_key: SecretKey,
    pub public_signing_key: Option<PublicKey>,
    pub public_encryption_key: Option<PublicKey>,
    pub private_signing_key: Option<SecretKey>,
    pub private_encryption_key: Option<SecretKey>,
}

impl Address {
    pub fn new(ripe: Vec<u8>) -> Self {
        let checksum = Sha512::digest(Sha512::digest(&ripe));
        let public_decryption_key = SecretKey::parse_slice(&checksum[..32]).unwrap();
        let tag = checksum[32..].to_vec();

        // FIXME make checksum of ripe
        let string_repr = bs58::encode(&ripe).into_string();
        Address {
            label: "".to_string(),
            ripe,
            tag,
            public_decryption_key,
            public_signing_key: Option::None,
            private_signing_key: Option::None,
            public_encryption_key: None,
            private_encryption_key: None,
            string_repr,
        }
    }

    pub fn with_public_key(
        public_signing_key: PublicKey,
        public_encryption_key: PublicKey,
    ) -> Self {
        let mut hasher = Sha512::new();
        let mut ripemd160 = Ripemd160::new();
        hasher.update(public_signing_key.serialize());
        hasher.update(public_encryption_key.serialize());
        ripemd160.update(hasher.finalize());
        let ripe = ripemd160.finalize().to_vec();

        let mut address = Self::new(ripe);
        address.public_signing_key = Some(public_signing_key);
        address.public_encryption_key = Some(public_encryption_key);
        return address;
    }

    pub fn with_private_key(private_signing_key: SecretKey, private_enc_key: SecretKey) -> Self {
        let psk = PublicKey::from_secret_key(&private_signing_key);
        let pek = PublicKey::from_secret_key(&private_enc_key);
        let mut address = Self::with_public_key(psk, pek);
        address.private_signing_key = Some(private_signing_key);
        address.private_encryption_key = Some(private_enc_key);
        address
    }

    pub fn with_string_repr(address: String) -> anyhow::Result<Self> {
        let ripe = bs58::decode(address)
            .into_vec()
            .map_err(|e| anyhow::anyhow!("string needs to be base58 encoded: {}", e))?;
        Ok(Self::new(ripe))
    }

    pub fn generate() -> Self {
        let psk = SecretKey::random(&mut OsRng);
        let pek = SecretKey::random(&mut OsRng);
        let address = Self::with_private_key(psk, pek);
        address
    }
}

#[allow(dead_code)]
pub fn get_leading(bytes: &[u8]) -> u32 {
    let mut zeros = 0;
    for &byte in bytes {
        zeros += byte.leading_zeros();
        if byte != 0 {
            break;
        }
    }

    zeros
}
