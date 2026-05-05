use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20poly1305::{ChaCha20Poly1305, aead::{Aead, KeyInit}};
use base64::Engine;
use rand::Rng;

pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    if hex.len() % 2 != 0 {
        return Err("Hex string has odd length".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn derive_name_nonce(key: &[u8], parent_path: &str) -> [u8; 12] {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(b"filename_nonce_v2");
    hasher.update(key);
    hasher.update(parent_path.as_bytes());
    let hash = hasher.finalize();
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&hash[..12]);
    nonce
}

fn encrypt_single_name(name: &str, password_hex: &str, parent_path: &str) -> Result<String, String> {
    let key_bytes = hex_to_bytes(password_hex)?;
    let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| "Key must be 32 bytes".to_string())?;
    let nonce = derive_name_nonce(&key_array, parent_path);
    let mut cipher = chacha20::ChaCha20::new(
        chacha20::Key::from_slice(&key_array),
        chacha20::Nonce::from_slice(&nonce),
    );
    let mut buf = name.as_bytes().to_vec();
    cipher.apply_keystream(&mut buf);
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf))
}

pub fn encrypt_filename(name: &str, password_hex: &str) -> Result<String, String> {
    let parts: Vec<&str> = name.split('/').collect();
    let mut encrypted_parts = Vec::with_capacity(parts.len());
    let mut parent = String::new();
    for part in parts {
        encrypted_parts.push(encrypt_single_name(part, password_hex, &parent)?);
        if !parent.is_empty() {
            parent.push('/');
        }
        parent.push_str(part);
    }
    Ok(encrypted_parts.join("/"))
}


fn derive_block_nonce(file_nonce: &[u8; 12], block_counter: u32) -> [u8; 12] {
    let mut nonce = *file_nonce;
    let counter_bytes = block_counter.to_le_bytes();
    for i in 0..4 {
        nonce[8 + i] ^= counter_bytes[i];
    }
    nonce
}

const BLOCK_HEADER_SIZE: usize = 4;

pub struct Encryptor {
    cipher: ChaCha20Poly1305,
    file_nonce: [u8; 12],
    nonce_emitted: bool,
    block_counter: u32,
}

impl Encryptor {
    pub fn new(password_hex: &str) -> Result<Self, String> {
        let key_bytes = hex_to_bytes(password_hex)?;
        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| "Key must be 32 bytes".to_string())?;
        let file_nonce: [u8; 12] = rand::thread_rng().gen();
        Ok(Encryptor {
            cipher: ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&key_array)),
            file_nonce,
            nonce_emitted: false,
            block_counter: 0,
        })
    }

    pub fn encrypt_block(&mut self, data: &[u8]) -> Result<Vec<u8>, String> {
        let nonce = derive_block_nonce(&self.file_nonce, self.block_counter);
        self.block_counter = self.block_counter.checked_add(1).ok_or("Block counter overflow")?;

        let ciphertext_with_tag = self.cipher
            .encrypt(chacha20poly1305::Nonce::from_slice(&nonce), data)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        let ciphertext_len = (data.len() as u32).to_be_bytes();

        let mut output = Vec::with_capacity(
            (if !self.nonce_emitted { 12 } else { 0 }) + BLOCK_HEADER_SIZE + ciphertext_with_tag.len(),
        );

        if !self.nonce_emitted {
            self.nonce_emitted = true;
            output.extend_from_slice(&self.file_nonce);
        }

        output.extend_from_slice(&ciphertext_len);
        output.extend_from_slice(&ciphertext_with_tag);
        Ok(output)
    }
}

pub struct Decryptor {
    cipher: Option<ChaCha20Poly1305>,
    key_bytes: [u8; 32],
    file_nonce: Option<[u8; 12]>,
    buffer: Vec<u8>,
    block_counter: u32,
}

impl Decryptor {
    pub fn new(password_hex: &str) -> Result<Self, String> {
        let key_bytes = hex_to_bytes(password_hex)?;
        let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| "Key must be 32 bytes".to_string())?;
        Ok(Decryptor {
            cipher: None,
            key_bytes: key_array,
            file_nonce: None,
            buffer: Vec::new(),
            block_counter: 0,
        })
    }

    pub fn decrypt_block(&mut self, data: &[u8]) -> Result<Vec<u8>, String> {
        self.buffer.extend_from_slice(data);

        if self.cipher.is_none() {
            if self.buffer.len() < 12 {
                return Ok(Vec::new());
            }
            let nonce: [u8; 12] = self.buffer[..12].try_into().map_err(|_| "Nonce extraction failed")?;
            self.file_nonce = Some(nonce);
            self.cipher = Some(ChaCha20Poly1305::new(chacha20poly1305::Key::from_slice(&self.key_bytes)));
            self.buffer = self.buffer[12..].to_vec();
        }

        let mut result = Vec::new();

        loop {
            if self.buffer.len() < BLOCK_HEADER_SIZE {
                break;
            }
            let ciphertext_len = u32::from_be_bytes(
                self.buffer[..BLOCK_HEADER_SIZE].try_into().map_err(|_| "Invalid block header")?,
            ) as usize;
            let total_block_size = BLOCK_HEADER_SIZE + ciphertext_len + 16;
            if self.buffer.len() < total_block_size {
                break;
            }

            let nonce = derive_block_nonce(self.file_nonce.as_ref().unwrap(), self.block_counter);
            self.block_counter = self.block_counter.checked_add(1).ok_or("Block counter overflow")?;

            let payload = &self.buffer[BLOCK_HEADER_SIZE..total_block_size];
            let plaintext = self.cipher
                .as_ref()
                .unwrap()
                .decrypt(chacha20poly1305::Nonce::from_slice(&nonce), payload)
                .map_err(|e| format!("Decryption failed at block {}: {}", self.block_counter, e))?;

            result.extend_from_slice(&plaintext);
            self.buffer = self.buffer[total_block_size..].to_vec();
        }

        Ok(result)
    }
}
