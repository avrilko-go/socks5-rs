use std::collections::HashMap;
use rand::seq::SliceRandom;
use rand::thread_rng;

#[derive(Debug, Clone)]
pub struct Cipher {
    encode_password: HashMap<u8, u8>,
    decode_password: HashMap<u8, u8>,
}

impl Cipher {
    pub fn new(password: String) -> Self {
        let v = base64::decode(password).unwrap();

        let mut encode_password = HashMap::new();
        let mut decode_password = HashMap::new();
        for (index, value) in v.iter().enumerate() {
            encode_password.insert(index as u8, *value);
            decode_password.insert(*value, index as u8);
        }

        Self {
            encode_password,
            decode_password,
        }
    }

    pub fn rand_password() -> String {
        let mut arr = vec![];
        for x in 0..=255 {
            arr.push(x);
        }
        let mut rng = thread_rng();
        arr.shuffle(&mut rng);
        base64::encode(arr)
    }

    pub fn encode(&self, data: &[u8], buffer: &mut Vec<u8>) {
        for u8 in data.iter() {
            buffer.push(self.encode_password[u8]);
        }
    }

    pub fn decode(&self, data: &[u8], buffer: &mut Vec<u8>) {
        for u8 in data.iter() {
            buffer.push(self.decode_password[u8]);
        }
    }
}
