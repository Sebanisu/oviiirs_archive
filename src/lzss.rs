pub use lzss::{compress, decompress};

pub mod lzss {
    use std::cell::RefCell;

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

    struct CompressImpl {
        left_side: Vec<u32>,
        right_side: Vec<u32>,
        parent: Vec<u32>,
        text_buf: Vec<u8>,
        match_length: usize,
        match_position: usize,
    }

    impl CompressImpl {
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

            while cmp >= 0 {
                if self.right_side[p] != NOT_USED {
                    p = self.right_side[p] as usize;
                } else {
                    self.right_side[p] = item as u32;
                    self.parent[item] = p as u32;
                    return;
                }

                cmp = key
                    .iter()
                    .zip(&self.text_buf[p..])
                    .find(|(&a, &b)| a != b)
                    .map_or(0, |(a, b)| *a as i16 - *b as i16);
            }

            self.match_position = p;
            if self.match_length >= NODE_SIZE {
                return;
            }

            self.match_length = NODE_SIZE;
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
        fn compress(&mut self, src: &[u8]) -> Vec<u8> {
            let mut cur_result = 0;
            let size_alloc = src.len() / 2;
            let mut code_buf = [0u8; F_MINUS1];
            let data_end = src.len();
            let mut data = 0;
            let mut result = Vec::with_capacity(size_alloc);

            // Initialize trees
            self.parent.iter_mut().for_each(|p| *p = NOT_USED);
            self.right_side[N_PLUS1..]
                .iter_mut()
                .for_each(|r| *r = NOT_USED);
            code_buf[0] = 0;
            let mut s = 0;
            let mut r = R_SIZE;

            // Fill text_buf with initial data
            self.text_buf
                .extend_from_slice(&src[data..data + R_SIZE as usize]);
            let mut len = 0;
            while len < NODE_SIZE as usize && data < data_end {
                self.text_buf[N + len] = src[data];
                len += 1;
                data += 1;
            }

            if len == 0 {
                result.clear();
                return result;
            }

            // Insert nodes for initial data
            for i in 1..=NODE_SIZE {
                self.insert_node(r as usize - i);
            }

            self.insert_node(r as usize);

            let mut code_buf_ptr = 1;
            let mut mask = 1;

            while len > 0 {
                if self.match_length > len as usize {
                    self.match_length = len as usize;
                }

                if self.match_length <= 2 {
                    self.match_length = 1;
                    code_buf[0] |= mask;
                    code_buf[code_buf_ptr] = self.text_buf[r as usize];
                    code_buf_ptr += 1;
                } else {
                    code_buf[code_buf_ptr] = self.match_position as u8;
                    code_buf_ptr += 1;
                    code_buf[code_buf_ptr] = ((self.match_position >> 4) as u32 & MATCH_MASK) as u8
                        | (self.match_length - (2 + 1)) as u8;
                    code_buf_ptr += 1;
                }

                if (mask << 1) == 0 {
                    result.extend_from_slice(&code_buf[..code_buf_ptr]);
                    cur_result += code_buf_ptr;
                    code_buf[0] = 0;
                    code_buf_ptr = 1;
                    mask = 1;
                }

                let last_match_length = self.match_length;
                for _ in 0..last_match_length {
                    if data == data_end {
                        break;
                    }

                    let c = src[data];
                    self.delete_node(s);
                    self.text_buf[s] = c;

                    if s < F_MINUS1 {
                        self.text_buf[s + N] = c;
                    }

                    s = (s + 1) & N_MINUS1;
                    r = (r + 1) & N_MINUS1;
                    self.insert_node(r);
                    data += 1;
                }

                while self.match_length > 0 {
                    self.delete_node(s);
                    s = (s + 1) & N_MINUS1;
                    r = (r + 1) & N_MINUS1;
                    if len > 0 {
                        self.insert_node(r);
                    }
                    self.match_length -= 1;
                }
            }

            if code_buf_ptr > 1 {
                result.extend_from_slice(&code_buf[..code_buf_ptr]);
                cur_result += code_buf_ptr;
            }

            result.truncate(cur_result);
            result
        }
    }

    pub fn compress(src: &[u8]) -> Vec<u8> {
        let mut compress_impl = CompressImpl::new();
        compress_impl.compress(src)
    }

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
                count = ((count & COUNT_MASK) + THRESHOLD as u32);

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
}
