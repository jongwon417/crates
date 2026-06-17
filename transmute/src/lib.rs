/// 배열을 구조체로 변경
pub fn transmute_ptr<T>(bin: &[u8]) -> &T {
    unsafe { &*bin.as_ptr().cast::<T>() }
}

/// 배열을 구조체로 변경
pub fn transmute_ptr_mut<T>(bin: &mut [u8]) -> &mut T {
    unsafe { &mut *bin.as_mut_ptr().cast::<T>() }
}

/// 구조체를 배열로 변경
pub fn transmute_to_array<T>(data: &T) -> &[u8] {
    unsafe { std::mem::transmute::<(&T, usize), &[u8]>((data, size_of::<T>())) }
}

/// 구조체를 배열로 변경
pub fn transmute_to_array_mut<T>(data: &mut T) -> &mut [u8] {
    unsafe { std::mem::transmute::<(&mut T, usize), &mut [u8]>((data, size_of::<T>())) }
}

/// 정렬되지 않은 메모리를 복사하여 반환
pub fn read_unaligned<T>(src: &[u8]) -> (T, &[u8])
where
    T: Copy,
{
    assert!(src.len() >= size_of::<T>());
    (unsafe { std::ptr::read_unaligned(src.as_ptr() as *const T) }, &src[size_of::<T>()..])
}

fn string_size(array: &[u8]) -> usize {
    let mut pos = 0;
    for a in array {
        if *a == 0 {
            return pos;
        }
        pos += 1;
    }
    pos
}

// if len is 0 -> return ""
pub fn array_to_string(array: &[u8]) -> String {
    let len = string_size(array);
    let mut out = String::new();
    if len > 0 {
        out = String::from_utf8_lossy(&array[..len]).to_string();
    }
    out
}

// if len is 0 -> return None
pub fn array_to_opt_string(array: &[u8]) -> Option<String> {
    let len = string_size(array);
    if len > 0 {
        Some(String::from_utf8_lossy(&array[..len]).to_string())
    } else {
        None
    }
}

macro_rules! string_to_array {
    ($name:ident, $size:expr) => {
        pub fn $name(src: &String) -> [u8; $size] {
            const MAX_LEN: usize = $size - 1;
            let mut array: [u8; $size] = [0u8; $size];
            let mut tmp = src.as_bytes().to_vec();
            let mut tmp_len = tmp.len();
            if tmp_len > MAX_LEN {
                tmp.truncate(MAX_LEN);
                tmp_len = MAX_LEN;
            }
            array[0..tmp_len].copy_from_slice(tmp.as_slice());
            array
        }
    };
}

string_to_array!(string_to_array16, 16);
string_to_array!(string_to_array20, 20);
string_to_array!(string_to_array32, 32);
string_to_array!(string_to_array40, 40);
string_to_array!(string_to_array64, 64);
string_to_array!(string_to_array128, 128);
string_to_array!(string_to_array256, 256);

macro_rules! opt_string_to_array {
    ($name:ident, $size:expr) => {
        pub fn $name(osrc: &Option<String>) -> [u8; $size] {
            const MAX_LEN: usize = $size - 1;
            let mut array: [u8; $size] = [0u8; $size];
            if let Some(src) = osrc {
                let mut tmp = src.as_bytes().to_vec();
                let mut tmp_len = tmp.len();
                if tmp_len > MAX_LEN {
                    tmp.truncate(MAX_LEN);
                    tmp_len = MAX_LEN;
                }
                array[0..tmp_len].copy_from_slice(tmp.as_slice());
            }
            array
        }
    };
}

opt_string_to_array!(opt_string_to_array16, 16);
opt_string_to_array!(opt_string_to_array20, 20);
opt_string_to_array!(opt_string_to_array32, 32);
opt_string_to_array!(opt_string_to_array40, 40);
opt_string_to_array!(opt_string_to_array64, 64);
opt_string_to_array!(opt_string_to_array128, 128);
opt_string_to_array!(opt_string_to_array256, 256);

pub fn null_string() -> String {
    String::new()
}

pub fn string_to_option(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub fn option_to_string(o: &Option<String>) -> String {
    if let Some(o) = o {
        o.clone()
    } else {
        "".to_string()
    }
}

// log level 미만이면 해당 인자는 평가되지 않는다.
// use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
// static mut CALLED: AtomicU64 = AtomicU64::new(0);
// pub fn called() -> u64 {
//     unsafe { CALLED.load(Ordering::SeqCst) }
// }

pub fn to_hex_string(array: &[u8]) -> String {
    let mut out = String::new();
    // unsafe { CALLED.fetch_add(1, Ordering::SeqCst) };

    for a in array {
        out.push_str(&format!("{:02x}", a));
    }
    out
}

pub fn to_pretty_hex_string(array: &[u8]) -> String {
    let mut pos = 0;
    let mut out = String::new();

    for a in array {
        out.push_str(&format!(" {:02x}", a));
        pos += 1;
        if pos == 8 {
            out.push('\n');
            pos = 0;
        }
    }
    out
}

macro_rules! define_read_be {
    ($( $name:ident : $ty:ty ),+ $(,)?) => {
        $(
            pub fn $name(input: &mut &[u8]) -> $ty {
                let (int_bytes, rest) = input.split_at(std::mem::size_of::<$ty>());
                *input = rest;
                <$ty>::from_be_bytes(int_bytes.try_into().unwrap())
            }
        )+
    };
}

define_read_be!(
    read_be_u8: u8,
    read_be_u16: u16,
    read_be_u32: u32,
    read_be_u64: u64,
    read_be_usize: usize,
    read_be_i8: i8,
    read_be_i16: i16,
    read_be_i32: i32,
    read_be_i64: i64,
    read_be_isize: isize,
);

macro_rules! define_read_le {
    ($( $name:ident : $ty:ty ),+ $(,)?) => {
        $(
            pub fn $name(input: &mut &[u8]) -> $ty {
                let (int_bytes, rest) = input.split_at(std::mem::size_of::<$ty>());
                *input = rest;
                <$ty>::from_le_bytes(int_bytes.try_into().unwrap())
            }
        )+
    };
}

define_read_le!(
    read_le_u8: u8,
    read_le_u16: u16,
    read_le_u32: u32,
    read_le_u64: u64,
    read_le_usize: usize,
    read_le_i8: i8,
    read_le_i16: i16,
    read_le_i32: i32,
    read_le_i64: i64,
    read_le_isize: isize,
);