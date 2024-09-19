pub use lzss::{compress, decompress};

pub mod lzss {
    use log::{debug, error, info, trace, warn};
    use std::{cell::RefCell, fmt};

    const R_SIZE: usize = 4078;
    const MATCH_MASK: u32 = 0xF0;
    const P_OFFSET: usize = 4097;
    const FLAGS_MASK: u32 = 0x100;
    const FLAGS_BITS: u32 = 0xFF00;
    const OFFSET_MASK: u32 = MATCH_MASK;
    const COUNT_MASK: u32 = 0x0F;
    const NOT_USED: u32 = 4096;
    const NODE_SIZE: usize = 18;
    const F: usize = 18;
    const F_MINUS1: usize = F - 1;
    const N: usize = NOT_USED as usize;
    const N_MINUS1: usize = N - 1;
    const THRESHOLD: usize = 2;
    const RIGHT_SIDE_SIZE: usize = 4353;
    const N_PLUS1: usize = N + 1;
    const N_PLUS2: usize = N + 2;
    const N_PLUS17: usize = N + F_MINUS1;

    fn decode_buffer(code_buf: &[u8]) -> String {
        code_buf
            .iter()
            .map(|b| {
                match b {
                    // If the byte represents a valid ASCII character, add it to the decoded string
                    0x20..=0x7E => (*b as char).to_string(),
                    // For invalid bytes, print them as \x followed by their hexadecimal representation
                    _ => format!("{:02X}", b).to_string(),
                }
            })
            .collect::<Vec<String>>()
            .join(", ")
    }

    #[derive(Debug)]
    pub struct CompressionError {
        pub message: String,
    }

    impl fmt::Display for CompressionError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.message)
        }
    }

    impl std::error::Error for CompressionError {}

    impl CompressionError {
        fn new(message: &str) -> Self {
            CompressionError {
                message: message.to_string(),
            }
        }
    }

    struct CompressImpl {
        left_side: [u32; N_PLUS2],
        right_side: [u32; RIGHT_SIDE_SIZE],
        parent: [u32; N_PLUS2],
        text_buf: [u8; N_PLUS17],
        match_length: usize,
        match_position: usize,
    }

    impl CompressImpl {
        #[allow(dead_code)]
        fn new() -> Self {
            let mut right_side = [0u32; RIGHT_SIDE_SIZE];
            right_side[N_PLUS1..].fill(NOT_USED);

            Self {
                left_side: [0; N_PLUS2],
                right_side: right_side,
                parent: [NOT_USED; N_PLUS2],
                text_buf: [0; N_PLUS17],
                match_length: 0,
                match_position: 0,
            }
        }

        //only should be called by compress
        fn insert_node(&mut self, item: usize) {
            let mut cmp: i16 = 1;
            let key = &self.text_buf[item..];
            let mut p = P_OFFSET + key[0] as usize;
            self.right_side[item] = NOT_USED;
            self.left_side[item] = NOT_USED;
            self.match_length = 0;

            loop {
                if cmp >= 0 {
                    if self.right_side[p] != NOT_USED {
                        p = self.right_side[p] as usize;
                    } else {
                        self.right_side[p] = item as u32;
                        self.parent[item] = p as u32;
                        return;
                    }
                } else {
                    if self.left_side[p] != NOT_USED {
                        p = self.left_side[p] as usize;
                    } else {
                        self.left_side[p] = item as u32;
                        self.parent[item] = p as u32;
                        return;
                    }
                }

                let mut node_index: usize = 1;
                for (new_index, new_cmp) in key
                    .iter()
                    .take(NODE_SIZE)
                    .skip(1)
                    .zip(self.text_buf.iter().skip(1 + p))
                    .map(|(a, b)| *a as i16 - *b as i16)
                    .enumerate()
                    .map(|(i, c)| (i + 1, c))
                {
                    cmp = new_cmp;
                    node_index = new_index;
                    if new_cmp == 0 {
                        break;
                    }
                }
                if node_index > self.match_length {
                    self.match_position = p;
                    self.match_length = node_index;
                    if self.match_length >= NODE_SIZE {
                        break;
                    }
                }
            }
            self.parent[item] = self.parent[p];
            self.left_side[item] = self.left_side[p];
            self.right_side[item] = self.right_side[p];
            self.parent[self.left_side[p] as usize] = item as u32;
            self.parent[self.right_side[p] as usize] = item as u32;
            if self.right_side[self.parent[p] as usize] == p as u32 {
                self.right_side[self.parent[p] as usize] = item as u32;
            } else {
                self.left_side[self.parent[p] as usize] = item as u32;
            }
            self.parent[p] = NOT_USED; // remove p
        }

        //only should be called by compress
        fn delete_node(&mut self, p: usize) {
            if self.parent[p] == NOT_USED {
                return;
            }

            let q = if self.right_side[p] == NOT_USED {
                self.left_side[p]
            } else if self.left_side[p] == NOT_USED {
                self.right_side[p]
            } else {
                let mut q_i = self.left_side[p];
                if self.right_side[q_i as usize] != NOT_USED {
                    loop {
                        q_i = self.right_side[q_i as usize];
                        if self.right_side[q_i as usize] == NOT_USED {
                            break;
                        }
                    }

                    self.right_side[self.parent[q_i as usize] as usize] =
                        self.left_side[q_i as usize];
                    self.parent[self.left_side[q_i as usize] as usize] = self.parent[q_i as usize];
                    self.left_side[q_i as usize] = self.left_side[p];
                    self.parent[self.left_side[p] as usize] = q_i;
                }

                self.right_side[q_i as usize] = self.right_side[p];
                self.parent[self.right_side[p] as usize] = q_i;
                q_i
            };

            self.parent[q as usize] = self.parent[p];
            if self.right_side[self.parent[p] as usize] as usize == p {
                self.right_side[self.parent[p] as usize] = q;
            } else {
                self.left_side[self.parent[p] as usize] = q;
            }
            self.parent[p] = NOT_USED;
        }

        #[allow(dead_code)]
        fn compress(mut self, src: &[u8]) -> Vec<u8> {
            // should only be called once
            if src.iter().peekable().peek().is_none() {
                info!("No data to compress. Returning empty result.");
                return Vec::new();
            }

            info!("Starting compression...");

            let mut result = Vec::with_capacity(src.len() / 2);
            let mut cur_result = 0;
            let mut code_buf = [0u8; F_MINUS1];
            let mut data = src.iter();
            let mut s = 0;
            let mut r = R_SIZE;
            let mut len = 0;

            info!("Initialized variables...");

            info!("Filling text_buff with the start of data.");
            while len < NODE_SIZE {
                match data.next() {
                    Some(symbol) => {
                        self.text_buf[r + len] = *symbol;
                        len += 1;
                    }
                    None => break,
                }
            }

            // info!("Checking if any data was inserted.");
            // if len == 0 {
            //     info!("No data to compress. Returning empty result.");
            //     result.clear();
            //     return result;
            // }

            info!("Inserted nodes for initial data...");

            for i in 1..=NODE_SIZE {
                self.insert_node(r - i);
            }
            self.insert_node(r);

            info!("Initialized nodes for compression...");

            let mut code_buf_ptr = 1;
            let mut mask = 1;

            info!("Starting loop iteration...");
            loop {
                if self.match_length > len {
                    self.match_length = len;
                    debug!("Match length adjusted...");
                }

                if self.match_length <= 2 {
                    self.match_length = 1;
                    code_buf[0] |= mask;
                    code_buf[code_buf_ptr] = self.text_buf[r];
                    code_buf_ptr += 1;
                    debug!(
                        "Match length is less than or equal to 2... \n\t\t{:?}\n\t\t{:?}",
                        code_buf,
                        decode_buffer(&code_buf)
                    );
                } else {
                    code_buf[code_buf_ptr] = self.match_position as u8;
                    code_buf_ptr += 1;
                    code_buf[code_buf_ptr] = ((self.match_position >> 4) as u32 & MATCH_MASK) as u8
                        | (self.match_length - (2 + 1)) as u8;
                    code_buf_ptr += 1;
                    debug!(
                        "Match length is greater than 2... \n\t\t{:?}\n\t\t{:?}",
                        code_buf,
                        decode_buffer(&code_buf)
                    );
                }

                if (mask << 1) != 0 {
                    mask = mask << 1;
                } else {
                    info!("Mask reached maximum value...");

                    result.extend_from_slice(&code_buf[..code_buf_ptr]);
                    //assert_eq!(result, verify[..result.len()]);

                    cur_result += code_buf_ptr;
                    code_buf[0] = 0;
                    code_buf_ptr = 1;
                    mask = 1;
                }

                info!("Result updated...");

                let last_match_length = self.match_length;
                let mut loop_count = 0;

                info!("Entering match processing loop...");
                for _ in 0..last_match_length {
                    let c = match data.next() {
                        Some(symbol) => symbol,
                        None => {
                            break;
                        }
                    };
                    self.delete_node(s);
                    self.text_buf[s] = *c;

                    self.text_buf[s + N] = *c;

                    s = (s + 1) & N_MINUS1;
                    r = (r + 1) & N_MINUS1;
                    self.insert_node(r);

                    loop_count += 1;
                }

                info!("Match processed...");

                info!("Processing remaining matches...");
                for _ in loop_count..last_match_length {
                    self.delete_node(s);
                    s = (s + 1) & N_MINUS1;
                    r = (r + 1) & N_MINUS1;
                    if len > 0 {
                        self.insert_node(r);
                        len -= 1;
                    }
                }

                info!("Remaining matches processed...");

                if len == 0 {
                    info!("No more data to compress. Exiting loop...");
                    break;
                }
            }

            info!("Compression loop completed...");

            if code_buf_ptr > 1 {
                result.extend_from_slice(&code_buf[..code_buf_ptr]);
                cur_result += code_buf_ptr;
            }

            result.truncate(cur_result);

            //assert_eq!(result, verify[..result.len()]);
            info!("Compression successful. Returning result.");
            result
        }
    }

    #[allow(dead_code)]
    pub fn compress(src: &[u8]) -> Vec<u8> {
        CompressImpl::new().compress(src)
    }

    #[allow(dead_code)]
    pub fn decompress(src: &[u8], dst_size: usize) -> Vec<u8> {
        let mut dst = Vec::<u8>::new();
        if dst_size > 0 {
            dst.reserve(dst_size);
        }

        let iterator = RefCell::new(src.iter());
        let mut text_buf = [0u32; N_MINUS1 + F];
        let mut r = N - F;
        let mut flags = 0u32;

        let test_at_end = || iterator.borrow().as_slice().is_empty();
        let next = || iterator.borrow_mut().next().cloned().unwrap_or(0u8);

        while !test_at_end() {
            flags >>= 1;
            if flags & FLAGS_MASK == 0 {
                if test_at_end() {
                    break;
                }
                flags = next() as u32 | FLAGS_BITS;
            }

            if flags & 1 == 1 {
                if test_at_end() {
                    break;
                }
                let current = next() as u32;
                dst.push(current as u8);
                text_buf[r] = current;
                r = (r + 1) & N_MINUS1;
            } else {
                if test_at_end() {
                    break;
                }
                let offset = next();
                if test_at_end() {
                    break;
                }
                let mut count = next() as u32;
                let offset = (offset as u32 | ((count as u32 & OFFSET_MASK) << 4)) as u32;
                count = (count & COUNT_MASK) + THRESHOLD as u32;

                for k in 0..=count {
                    let current = text_buf[(offset as usize + k as usize) & N_MINUS1];
                    dst.push(current as u8);
                    text_buf[r] = current;
                    r = (r + 1) & N_MINUS1;
                }
            }
        }

        dst
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_compress_decompress() {
            let _ = env_logger::builder().is_test(true).try_init();
            // Test data
            let test_data: [(&str, Vec<u8>); 11] = [
                ("", vec![]),
                ("Hello", vec![31, 72, 101, 108, 108, 111]),
                (
                    "1234567890",
                    vec![255, 49, 50, 51, 52, 53, 54, 55, 56, 3, 57, 48],
                ),
                (
                    "Lorem ips",
                    vec![255, 76, 111, 114, 101, 109, 32, 105, 112, 1, 115],
                ),
                (
                    "Lorem ipsum d",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 31, 115, 117, 109, 32, 100,
                    ],
                ),
                (
                    "Lorem ipsum do",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 63, 115, 117, 109, 32, 100, 111,
                    ],
                ),
                (
                    "Lorem ipsum dol",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 127, 115, 117, 109, 32, 100,
                        111, 108,
                    ],
                ),
                (
                    "Lorem ipsum dolor",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 255, 115, 117, 109, 32, 100,
                        111, 108, 111, 1, 114,
                    ],
                ),
                (
                    "Lorem ipsum dolor s",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 255, 115, 117, 109, 32, 100,
                        111, 108, 111, 7, 114, 32, 115,
                    ],
                ),
                (
                    "Lorem ipsum dolor sit",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 255, 115, 117, 109, 32, 100,
                        111, 108, 111, 31, 114, 32, 115, 105, 116,
                    ],
                ),
                (
                    "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
                    vec![
                        255, 76, 111, 114, 101, 109, 32, 105, 112, 255, 115, 117, 109, 32, 100,
                        111, 108, 111, 255, 114, 32, 115, 105, 116, 32, 97, 109, 255, 101, 116, 44,
                        32, 99, 111, 110, 115, 255, 101, 99, 116, 101, 116, 117, 114, 32, 255, 97,
                        100, 105, 112, 105, 115, 99, 105, 255, 110, 103, 32, 101, 108, 105, 116,
                        46,
                    ],
                ),
            ];

            // Iterate over each original data value
            for (original_data, compressed_verification_data) in test_data.as_ref() {
                // Compress the data
                let compressed_data = compress(original_data.as_bytes());
                assert_eq!(&compressed_data, compressed_verification_data);

                // Decompress the data
                let decompressed_data = decompress(&compressed_data, original_data.len());

                // Assert that decompressed data matches original data
                assert_eq!(
                    std::str::from_utf8(&decompressed_data).unwrap(),
                    std::str::from_utf8(original_data.as_bytes()).unwrap()
                );
            }
        }
    }
}
