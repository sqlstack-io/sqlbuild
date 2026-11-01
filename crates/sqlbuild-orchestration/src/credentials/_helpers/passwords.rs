//! Memory-hard password work runs outside the async executor and outside write transactions.

use argon2::Argon2;
use argon2::password_hash::{Error, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

use crate::credentials::constants::{PASSWORD_ALGORITHM, PASSWORD_KDF_SLOTS};
use crate::store::errors::ControlError;

static PASSWORD_WORK: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(PASSWORD_KDF_SLOTS);

pub(crate) async fn hash_password(password: String) -> Result<String, ControlError> {
    let permit = PASSWORD_WORK
        .acquire()
        .await
        .map_err(|_| ControlError::Invalid("password workers are unavailable".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .map_err(|_| ControlError::Invalid("could not generate a password salt".into()))?;
        let salt = SaltString::encode_b64(&random)
            .map_err(|_| ControlError::Invalid("could not encode a password salt".into()))?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| ControlError::Invalid("could not hash the password".into()))
    })
    .await?
}

pub(crate) async fn verify_hash(hash: String, password: String) -> Result<bool, ControlError> {
    let permit = PASSWORD_WORK
        .acquire()
        .await
        .map_err(|_| ControlError::Invalid("password workers are unavailable".into()))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let hash = PasswordHash::new(&hash)
            .map_err(|_| ControlError::Invalid("stored password credential is invalid".into()))?;
        if hash.algorithm.as_str() != PASSWORD_ALGORITHM {
            return Err(ControlError::Invalid(
                "stored password credential is not Argon2id".into(),
            ));
        }
        match Argon2::default().verify_password(password.as_bytes(), &hash) {
            Ok(()) => Ok(true),
            Err(Error::Password) => Ok(false),
            Err(_) => Err(ControlError::Invalid(
                "stored password credential could not be verified".into(),
            )),
        }
    })
    .await?
}
