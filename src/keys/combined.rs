//! An implementation that combines the currently supported key types. This
//! facilitates and ENR type than can decode and read ENR's of all supported key types.
//!
//! Currently only `secp256k1` and `ed25519` key types are supported.

use super::{ed25519_dalek as ed25519, EnrKey, EnrPublicKey, SigningError};
use rand::RngCore;
use rlp::DecoderError;
pub use secp256k1;
use std::collections::BTreeMap;
use zeroize::Zeroize;

/// A standard implementation of the `EnrKey` trait used to sign and modify ENR records. The variants here represent the currently
/// supported in-built signing schemes.
pub enum CombinedKey {
    /// An `secp256k1` keypair.
    Secp256k1(secp256k1::SecretKey),
    /// An `Ed25519` keypair.
    Ed25519(ed25519::Keypair),
}

impl From<secp256k1::SecretKey> for CombinedKey {
    fn from(secret_key: secp256k1::SecretKey) -> CombinedKey {
        CombinedKey::Secp256k1(secret_key)
    }
}

impl From<ed25519::Keypair> for CombinedKey {
    fn from(keypair: ed25519_dalek::Keypair) -> CombinedKey {
        CombinedKey::Ed25519(keypair)
    }
}

/// Promote an Ed25519 secret key into a keypair.
impl From<ed25519::SecretKey> for CombinedKey {
    fn from(secret: ed25519::SecretKey) -> CombinedKey {
        let public = ed25519::PublicKey::from(&secret);
        CombinedKey::Ed25519(ed25519::Keypair { secret, public })
    }
}

impl EnrKey for CombinedKey {
    type PublicKey = CombinedPublicKey;

    /// Performs ENR-specific signing.
    ///
    /// Note: that this library supports a number of signing algorithms. The ENR specification
    /// currently lists the `v4` identity scheme which requires the `secp256k1` signing algorithm.
    /// Using `secp256k1` keys follow the `v4` identity scheme, using other types do not, although
    /// they are supported.
    fn sign_v4(&self, msg: &[u8]) -> Result<Vec<u8>, SigningError> {
        match self {
            CombinedKey::Secp256k1(ref key) => key.sign_v4(msg),
            CombinedKey::Ed25519(ref key) => key.sign_v4(msg),
        }
    }

    /// Returns the public key associated with the private key.
    fn public(&self) -> Self::PublicKey {
        match self {
            CombinedKey::Secp256k1(key) => CombinedPublicKey::from(key.public()),
            CombinedKey::Ed25519(key) => CombinedPublicKey::from(key.public()),
        }
    }

    /// Decodes the raw bytes of an ENR's content into a public key if possible.
    fn enr_to_public(content: &BTreeMap<String, Vec<u8>>) -> Result<Self::PublicKey, DecoderError> {
        secp256k1::SecretKey::enr_to_public(content)
            .map(CombinedPublicKey::Secp256k1)
            .or_else(|_| ed25519::Keypair::enr_to_public(content).map(CombinedPublicKey::from))
    }
}

impl CombinedKey {
    /// Generates a new secp256k1 key.
    pub fn generate_secp256k1() -> Self {
        let mut r = rand::thread_rng();
        let mut b = [0; secp256k1::util::SECRET_KEY_SIZE];
        // This is how it is done in `secp256k1::SecretKey::random` which
        // we do not use here because it uses `rand::Rng` from rand-0.4.
        loop {
            r.fill_bytes(&mut b);
            if let Ok(k) = secp256k1::SecretKey::parse(&b) {
                b.zeroize();
                return CombinedKey::Secp256k1(k);
            }
        }
    }

    /// Generates a new ed25510 key.
    pub fn generate_ed25519() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let key =
            CombinedKey::from(ed25519::SecretKey::from_bytes(&bytes).expect(
                "this returns `Err` only if the length is wrong; the length is correct; qed",
            ));
        bytes.zeroize();
        key
    }

    /// Imports a secp256k1 from raw bytes in any format.
    pub fn secp256k1_from_bytes(bytes: &mut [u8]) -> Result<Self, DecoderError> {
        let key = secp256k1::SecretKey::parse_slice(bytes)
            .map_err(|_| DecoderError::Custom("Invalid secp256k1 secret key"))
            .map(CombinedKey::from)?;
        bytes.zeroize();
        Ok(key)
    }

    /// Imports an ed25519 key from raw 32 bytes.
    pub fn ed25519_from_bytes(bytes: &mut [u8]) -> Result<Self, DecoderError> {
        let key = ed25519::SecretKey::from_bytes(bytes)
            .map_err(|_| DecoderError::Custom("Invalid ed25519 secret key"))
            .map(CombinedKey::from)?;
        bytes.zeroize();
        Ok(key)
    }

    /// Encodes the `CombinedKey` into compressed (where possible) bytes.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            CombinedKey::Secp256k1(key) => key.serialize().to_vec(),
            CombinedKey::Ed25519(key) => key.secret.as_bytes().to_vec(),
        }
    }
}

/// A combined implementation of `EnrPublicKey` which has support for `Secp256k1`
/// and `Ed25519` for ENR signature verification.
#[derive(Clone, Debug, PartialEq)]
pub enum CombinedPublicKey {
    /// An `Secp256k1` public key.
    Secp256k1(secp256k1::PublicKey),
    /// An `Ed25519` public key.
    Ed25519(ed25519::PublicKey),
}

impl From<secp256k1::PublicKey> for CombinedPublicKey {
    fn from(public_key: secp256k1::PublicKey) -> CombinedPublicKey {
        CombinedPublicKey::Secp256k1(public_key)
    }
}

impl From<ed25519::PublicKey> for CombinedPublicKey {
    fn from(public_key: ed25519::PublicKey) -> CombinedPublicKey {
        CombinedPublicKey::Ed25519(public_key)
    }
}

impl EnrPublicKey for CombinedPublicKey {
    /// Verify a raw message, given a public key for the v4 identity scheme.
    fn verify_v4(&self, msg: &[u8], sig: &[u8]) -> bool {
        match self {
            Self::Secp256k1(pk) => pk.verify_v4(msg, sig),
            Self::Ed25519(pk) => pk.verify_v4(msg, sig),
        }
    }

    /// Encodes the public key into compressed form, if possible.
    fn encode(&self) -> Vec<u8> {
        match self {
            // serialize in compressed form: 33 bytes
            Self::Secp256k1(pk) => pk.encode(),
            Self::Ed25519(pk) => pk.encode(),
        }
    }

    /// Encodes the public key in uncompressed form.
    fn encode_uncompressed(&self) -> Vec<u8> {
        match self {
            Self::Secp256k1(pk) => pk.encode_uncompressed(),
            Self::Ed25519(pk) => pk.encode_uncompressed(),
        }
    }

    /// Generates the ENR public key string associated with the key type.
    fn enr_key(&self) -> String {
        match self {
            Self::Secp256k1(key) => key.enr_key(),
            Self::Ed25519(key) => key.enr_key(),
        }
    }
}
