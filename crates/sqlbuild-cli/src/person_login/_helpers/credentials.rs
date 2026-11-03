//! Prefer the OS credential store; headless Unix fallback is private, owner-only and atomic.

use crate::person_login::constants::KEYRING_SERVICE;
use crate::person_login::models::PersonCredential;

fn headless_linux() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none()
        && std::env::var_os("XDG_RUNTIME_DIR").is_none()
}

pub(crate) fn load(origin: &str) -> Result<Option<PersonCredential>, std::io::Error> {
    let encoded = match keyring::Entry::new(KEYRING_SERVICE, origin)
        .and_then(|entry| entry.get_password())
    {
        Ok(value) => Some(value),
        Err(keyring::Error::NoEntry) => private_file(origin, None)?,
        Err(keyring::Error::PlatformFailure(_)) if headless_linux() => private_file(origin, None)?,
        Err(_) => {
            return Err(std::io::Error::other(
                "OS credential store could not be read; unlock it before retrying",
            ));
        }
    };
    encoded
        .map(|value| {
            serde_json::from_str(&value).map_err(|_| {
                std::io::Error::other("stored CLI credential is invalid; run sqb logout")
            })
        })
        .transpose()
}

pub(crate) fn save(origin: &str, credential: &PersonCredential) -> Result<bool, std::io::Error> {
    let encoded = serde_json::to_string(credential)
        .map_err(|_| std::io::Error::other("could not encode the CLI credential"))?;
    match keyring::Entry::new(KEYRING_SERVICE, origin)
        .and_then(|entry| entry.set_password(&encoded))
    {
        Ok(()) => {
            remove_file(origin)?;
            Ok(true)
        }
        Err(keyring::Error::PlatformFailure(_)) if headless_linux() => {
            private_file(origin, Some(&encoded))?;
            Ok(false)
        }
        Err(_) => Err(std::io::Error::other(
            "OS credential store rejected persistence; unlock it before retrying",
        )),
    }
}

pub(crate) fn remove(origin: &str) -> Result<(), std::io::Error> {
    let removed = match keyring::Entry::new(KEYRING_SERVICE, origin)
        .and_then(|entry| entry.delete_credential())
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(keyring::Error::PlatformFailure(_)) if headless_linux() => Ok(()),
        Err(_) => Err(std::io::Error::other(
            "OS credential store cleanup could not be confirmed",
        )),
    };
    remove_file(origin)?;
    removed
}

#[cfg(target_os = "linux")]
fn path(origin: &str, create: bool) -> Result<Option<std::path::PathBuf>, std::io::Error> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) else {
        return if create {
            Err(std::io::Error::other(
                "a user home directory is required for credential storage",
            ))
        } else {
            Ok(None)
        };
    };
    let mut directory = std::path::PathBuf::from(home);
    if !directory.is_absolute() {
        return Err(std::io::Error::other(
            "the credential home directory must be absolute",
        ));
    }
    let credential = directory
        .join(".config/sqlbuild/credentials")
        .join(format!("{}.json", blake3::hash(origin.as_bytes()).to_hex()));
    if !create {
        match std::fs::symlink_metadata(&credential) {
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    for (component, forbidden) in [
        ("", 0o022),
        (".config", 0o022),
        ("sqlbuild", 0o022),
        ("credentials", 0o077),
    ] {
        if !component.is_empty() {
            directory.push(component);
        }
        if create && !component.is_empty() {
            match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error),
            }
        }
        let metadata = std::fs::symlink_metadata(&directory)?;
        if !metadata.is_dir()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & forbidden != 0
        {
            return Err(std::io::Error::other(
                "CLI credential directories must be owned by you, not symlinks or writable by others; the credential directory must be private",
            ));
        }
    }
    Ok(Some(credential))
}

#[cfg(target_os = "linux")]
fn private_file(origin: &str, encoded: Option<&str>) -> Result<Option<String>, std::io::Error> {
    use std::io::{Read, Write};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let Some(path) = path(origin, encoded.is_some())? else {
        return Ok(None);
    };
    if let Some(encoded) = encoded {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::from_bytes(bytes)));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(encoded.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temporary, &path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result?;
        return Ok(None);
    }
    let mut file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o077 != 0
    {
        return Err(std::io::Error::other(
            "CLI credential file must be owned by you and accessible only to you",
        ));
    }
    let mut encoded = String::new();
    file.read_to_string(&mut encoded)?;
    Ok(Some(encoded))
}

#[cfg(target_os = "linux")]
fn remove_file(origin: &str) -> Result<bool, std::io::Error> {
    let Some(path) = path(origin, false)? else {
        return Ok(false);
    };
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(not(target_os = "linux"))]
fn private_file(_origin: &str, encoded: Option<&str>) -> Result<Option<String>, std::io::Error> {
    match encoded {
        None => Ok(None),
        Some(_) => Err(std::io::Error::other(
            "an available OS credential store is required on this platform",
        )),
    }
}

#[cfg(not(target_os = "linux"))]
fn remove_file(_origin: &str) -> Result<bool, std::io::Error> {
    Ok(false)
}
