extern crate libc;
extern crate dehtml;
extern crate serde_json;

use std::{slice, str, io, panic};
use libc::{c_int, size_t};
use serde_json::error::Error as SerdeError;

pub const ERR_UNSPECIFIED: c_int = -1;

pub const ERR_INSUFFICIENT_BUFFER: c_int = -2;

pub const ERR_INVALID_STRING: c_int = -3;

pub const ERR_INVALID_BAD_DOCUMENT: c_int = -4;

#[no_mangle]
pub unsafe extern "C" fn dehtml_errstr(errno: c_int) -> *const u8 {
    match errno {
        ERR_UNSPECIFIED => "unspecified\0".as_ptr(),
        ERR_INSUFFICIENT_BUFFER => "insufficient buffer\0".as_ptr(),
        ERR_INVALID_STRING => "invalid string\0".as_ptr(),
        ERR_INVALID_BAD_DOCUMENT => "invalid document\0".as_ptr(),
        _ => std::ptr::null(),
    }
}


#[no_mangle]
pub unsafe extern "C" fn parse_html(
    ibuf: *const u8,
    ilen: size_t,
    obuf: *mut u8,
    olen: size_t,
) -> c_int {
    if ibuf.is_null() {
        return ERR_UNSPECIFIED;
    }
    if obuf.is_null() {
        return ERR_UNSPECIFIED;
    }

    panic::catch_unwind(|| {
        let input = slice::from_raw_parts(ibuf, ilen);
        let output = slice::from_raw_parts_mut(obuf, olen);

        let input = match str::from_utf8(input) {
            Ok(input) => input,
            Err(_) => return ERR_INVALID_STRING,
        };

        let node = match dehtml::parse_html(input) {
            Ok(node) => node,
            Err(_err) => return ERR_INVALID_BAD_DOCUMENT,
        };
        let mut wri = io::Cursor::new(&mut output[..]);
        match serde_json::ser::to_writer(&mut wri, &node) {
            Err(SerdeError::Syntax(_, _, _)) => return ERR_UNSPECIFIED,
            Err(SerdeError::Io(_)) => return ERR_INSUFFICIENT_BUFFER,
            Err(SerdeError::FromUtf8(_)) => return ERR_UNSPECIFIED,
            Ok(()) => (),
        };
        wri.position() as c_int
    }).unwrap_or_else(|_| ERR_UNSPECIFIED)
}
