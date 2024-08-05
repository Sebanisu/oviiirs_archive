pub use lzss::{compress, decompress};

pub mod lzss {
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
    //const N_PLUS1: usize = N + 1;
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
        left_side: Vec<u32>,
        right_side: Vec<u32>,
        parent: Vec<u32>,
        text_buf: Vec<u8>,
        match_length: usize,
        match_position: usize,
    }

    impl CompressImpl {
        #[allow(dead_code)]
        fn new() -> Self {
            let left_side = vec![NOT_USED; N_PLUS2];
            let right_side = vec![NOT_USED; RIGHT_SIDE_SIZE];
            let parent = vec![NOT_USED; N_PLUS2];
            let text_buf = vec![0; N_PLUS17];
            let match_length = 0;
            let match_position = 0;

            CompressImpl {
                left_side,
                right_side,
                parent,
                text_buf,
                match_length,
                match_position,
            }
        }

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
        }
        #[allow(dead_code)]
        fn compress(&mut self, src: &[u8], verify: &[u8]) -> Result<Vec<u8>, CompressionError> {
            println!("Starting compression...");

            let mut cur_result = 0;
            let size_alloc = src.len() / 2;
            let mut code_buf = [0u8; F_MINUS1];
            let mut data = src.iter();
            let mut result = Vec::with_capacity(size_alloc);

            println!("Initialized variables...");

            let mut s = 0;
            let mut r = R_SIZE;
            let mut len = 0;

            println!("Entering main compression loop...");

            while len < NODE_SIZE as usize && data.len() > 0 {
                match data.next() {
                    Some(symbol) => {
                        self.text_buf[r + len] = *symbol;
                        len += 1;
                    }
                    None => break,
                }
            }

            if len == 0 {
                println!("No data to compress. Returning empty result.");
                result.clear();
                return Ok(result);
            }

            println!("Inserted nodes for initial data...");

            for i in 1..=NODE_SIZE {
                self.insert_node(r as usize - i);
            }
            self.insert_node(r as usize);

            println!("Initialized nodes for compression...");

            let mut code_buf_ptr = 1;
            let mut mask = 1;

            println!("Starting loop iteration...");
            loop {
                if self.match_length > len as usize {
                    self.match_length = len as usize;
                }

                println!("Match length adjusted...");

                if self.match_length <= 2 {
                    self.match_length = 1;
                    code_buf[0] |= mask;
                    code_buf[code_buf_ptr] = self.text_buf[r as usize];
                    code_buf_ptr += 1;
                    println!(
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
                    println!(
                        "Match length is greater than 2... \n\t\t{:?}\n\t\t{:?}",
                        code_buf,
                        decode_buffer(&code_buf)
                    );
                }

                if mask < 1 << 7 {
                    mask = mask << 1;
                } else {
                    println!("Mask reached maximum value...");

                    result.extend_from_slice(&code_buf[..code_buf_ptr]);
                    assert_eq!(result, verify[..result.len()]);

                    cur_result += code_buf_ptr;
                    code_buf[0] = 0;
                    code_buf_ptr = 1;
                    mask = 1;
                }

                println!("Result updated...");

                let last_match_length = self.match_length;
                let mut loop_count = 0;

                println!("Entering match processing loop...");
                for _ in 0..last_match_length {
                    let c = match data.next() {
                        Some(symbol) => symbol,
                        None => {
                            break;
                        }
                    };
                    self.delete_node(s);
                    self.text_buf[s] = *c;

                    if s < F_MINUS1 {
                        self.text_buf[s + N] = *c;
                    }

                    s = (s + 1) & N_MINUS1;
                    r = (r + 1) & N_MINUS1;
                    self.insert_node(r);

                    loop_count += 1;
                }

                println!("Match processed...");

                println!("Processing remaining matches...");
                for _ in loop_count..last_match_length {
                    self.delete_node(s);
                    s = (s + 1) & N_MINUS1;
                    r = (r + 1) & N_MINUS1;
                    if len > 0 {
                        self.insert_node(r);
                        len -= 1;
                    }
                }

                println!("Remaining matches processed...");

                if len == 0 {
                    println!("No more data to compress. Exiting loop...");
                    break;
                }
            }

            println!("Compression loop completed...");

            if code_buf_ptr > 1 {
                result.extend_from_slice(&code_buf[..code_buf_ptr]);
                cur_result += code_buf_ptr;
            }

            result.truncate(cur_result);

            assert_eq!(result, verify[..result.len()]);
            println!("Compression successful. Returning result.");
            Ok(result)
        }
    }
    #[allow(dead_code)]
    pub fn compress(src: &[u8], verify: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut compress_impl = CompressImpl::new();
        compress_impl.compress(src, verify)
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
            // Test data
            let original_data_values = [
                //"",                                                         // Empty string
                //"Hello",                                                    // String with length 5
                //"1234567890",                                               // String with length 10
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit.", // Longer string
            ];
            let compressed_verification_values: [Vec<u8>; 1] = [
                //vec![],
                //vec![31, 72, 101, 108, 108, 111],
                //vec![255, 49, 50, 51, 52, 53, 54, 55, 56, 3, 57, 48],
                vec![
                    255, 76, 111, 114, 101, 109, 32, 105, 112, 255, 115, 117, 109, 32, 100, 111,
                    108, 111, 255, 114, 32, 115, 105, 116, 32, 97, 109, 255, 101, 116, 44, 32, 99,
                    111, 110, 115, 255, 101, 99, 116, 101, 116, 117, 114, 32, 255, 97, 100, 105,
                    112, 105, 115, 99, 105, 255, 110, 103, 32, 101, 108, 105, 116, 46,
                ],
            ];

            // Iterate over each original data value
            for (original_data, compressed_verification_data) in original_data_values
                .iter()
                .map(|s| s.as_bytes())
                .zip(compressed_verification_values.iter())
            {
                // Compress the data
                let compressed_data = match compress(original_data, compressed_verification_data) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Compression error: {}", e);
                        panic!("Compression failed");
                    }
                };
                assert_eq!(&compressed_data, compressed_verification_data);

                // Decompress the data
                let decompressed_data = decompress(&compressed_data, original_data.len());

                // Assert that decompressed data matches original data
                assert_eq!(
                    std::str::from_utf8(&decompressed_data).unwrap(),
                    std::str::from_utf8(original_data).unwrap()
                );
            }
        }
    }
}
