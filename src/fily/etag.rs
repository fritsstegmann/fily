use md5::{Digest, Md5};

pub fn generate_etag(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("\"{}\"", hex::encode(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etag_generation() {
        let data = b"Hello, world!";
        let etag = generate_etag(data);
        assert_eq!(etag, "\"6cd3556deb0da54bca060b4c39479839\"");
    }

    #[test]
    fn test_etag_empty_data() {
        let data = b"";
        let etag = generate_etag(data);
        assert_eq!(etag, "\"d41d8cd98f00b204e9800998ecf8427e\"");
    }
}