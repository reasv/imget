use sha2::{Sha256, Digest};

pub fn hash_u8_array(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    
    let result = hasher.finalize();
    
    // Convert the hash to a hexadecimal string representation
    let hash_string = result.iter()
        .map(|byte| format!("{:02x}", byte))
        .collect::<String>();
    
    hash_string
}