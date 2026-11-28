use std::io;

use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::{abi, storage, sys};

const FILES: [&str; 2] = ["users-a.dat", "users-b.dat"];
const KDF_ROUNDS: u32 = 600_000;
const RECORD: usize = 80;
const LIMIT: usize = 16;

pub struct User {
    pub account: abi::Account,
    salt: [u8; 16],
    password: [u8; 32],
}

impl User {
    pub fn name(&self) -> &str {
        let length = self.account.name.iter().position(|&c| c == 0).unwrap_or(0);
        std::str::from_utf8(&self.account.name[..length]).unwrap_or("")
    }
}

pub struct Users(Vec<User>);

impl Users {
    pub fn load() -> io::Result<Self> {
        let Some(data) = storage::load(FILES)? else {
            return Ok(Self(Vec::new()));
        };
        let users = Self::decode(&data)?;
        for user in &users.0 {
            sys::register(&user.account)?;
        }
        Ok(users)
    }

    fn decode(data: &[u8]) -> io::Result<Self> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid account database");
        let records = data.chunks_exact(RECORD);
        if !records.remainder().is_empty() || records.len() == 0 || records.len() > LIMIT {
            return Err(invalid());
        }
        let mut users = Vec::new();
        for record in records {
            let user = User {
                account: abi::Account {
                    uid: u32::from_le_bytes(record[..4].try_into().unwrap()),
                    admin: record[4],
                    name: record[5..29].try_into().unwrap(),
                    reserved: [0; 3],
                },
                salt: record[32..48].try_into().unwrap(),
                password: record[48..80].try_into().unwrap(),
            };
            if !valid_name(user.name())
                || user.account.admin > 1
                || !(1000..=65535).contains(&user.account.uid)
                || users.iter().any(|entry: &User| {
                    entry.name() == user.name() || entry.account.uid == user.account.uid
                })
            {
                return Err(invalid());
            }
            users.push(user);
        }
        if !users.iter().any(|user| user.account.admin == 1) {
            return Err(invalid());
        }
        Ok(Self(users))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn create(&mut self, name: &str, password: &[u8], admin: bool) -> io::Result<()> {
        if !valid_name(name) {
            return Err(io::Error::other(
                "Use 1 to 23 lowercase letters, digits, or hyphens",
            ));
        }
        if !(8..=64).contains(&password.len()) {
            return Err(io::Error::other("Use a password of 8 to 64 characters"));
        }
        if self.0.len() >= LIMIT || self.0.iter().any(|user| user.name() == name) {
            return Err(io::Error::other(
                "Account already exists or the account limit was reached",
            ));
        }
        let mut name_bytes = [0; 24];
        name_bytes[..name.len()].copy_from_slice(name.as_bytes());
        let uid = self
            .0
            .iter()
            .map(|user| user.account.uid)
            .max()
            .unwrap_or(999)
            + 1;
        let account = abi::Account {
            uid,
            admin: u8::from(admin),
            name: name_bytes,
            reserved: [0; 3],
        };
        let mut salt = [0; 16];
        sys::random(&mut salt)?;
        let password = hash(password, &salt);
        self.0.push(User {
            account,
            salt,
            password,
        });
        if let Err(error) = self.save() {
            self.0.pop();
            return Err(error);
        }
        sys::register(&account)
    }

    fn save(&self) -> io::Result<()> {
        let mut data = Vec::with_capacity(self.0.len() * RECORD);
        for user in &self.0 {
            data.extend_from_slice(&user.account.uid.to_le_bytes());
            data.push(user.account.admin);
            data.extend_from_slice(&user.account.name);
            data.extend_from_slice(&[0; 3]);
            data.extend_from_slice(&user.salt);
            data.extend_from_slice(&user.password);
        }
        storage::save(FILES, &data)
    }

    pub fn change_password(&mut self, uid: u32, password: &[u8]) -> io::Result<()> {
        if !(8..=64).contains(&password.len()) {
            return Err(io::Error::other("Use a password of 8 to 64 characters"));
        }
        let index = self
            .0
            .iter()
            .position(|user| user.account.uid == uid)
            .ok_or_else(|| io::Error::other("Account no longer exists"))?;
        let mut salt = [0; 16];
        sys::random(&mut salt)?;
        let previous_salt = std::mem::replace(&mut self.0[index].salt, salt);
        let previous_hash = std::mem::replace(&mut self.0[index].password, hash(password, &salt));
        if let Err(error) = self.save() {
            self.0[index].salt = previous_salt;
            self.0[index].password = previous_hash;
            return Err(error);
        }
        Ok(())
    }

    pub fn authenticate(&self, name: &str, password: &[u8]) -> Option<abi::Account> {
        let user = self.0.iter().find(|user| user.name() == name);
        let computed = hash(password, &user.map_or([0; 16], |user| user.salt));
        user.filter(|user| bool::from(computed.ct_eq(&user.password)))
            .map(|user| user.account)
    }
}

fn valid_name(name: &str) -> bool {
    let name = name.as_bytes();
    !name.is_empty()
        && name.len() < 24
        && name != b"root"
        && name[0].is_ascii_lowercase()
        && name
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn hash(password: &[u8], salt: &[u8; 16]) -> [u8; 32] {
    let mut output = [0; 32];
    pbkdf2_hmac::<Sha256>(password, salt, KDF_ROUNDS, &mut output);
    output
}
