use std::{ffi::{CStr, c_char}, io, ops::Range};

#[derive(Debug, Clone)]
pub(crate) struct Uname {
    buf: String,
    sysname: Range<u16>,
    nodename: Range<u16>,
    release: Range<u16>,
    version: Range<u16>,
    machine: Range<u16>,
}

impl Uname {
    pub fn new() -> Self {
        let mut n: libc::utsname = unsafe { std::mem::zeroed() };
        if 0 != unsafe { libc::uname(&mut n) } {
            panic!("uname failed {}", io::Error::last_os_error())
        }

        let mut buf = String::with_capacity(size_of::<libc::utsname>());

        let mut add = |arr: [c_char; _]| {
            let arr = arr.map(|i| i as u8);
            let cstr = CStr::from_bytes_until_nul(arr.as_slice()).expect("uname(2) gave invalid data");
            let start = buf.len();
            buf.push_str(&cstr.to_string_lossy());
            let end = buf.len();
            start as u16..end as u16
        };

        macro_rules! field {
            ($($field:ident),* $(,)?) => {
                $(let $field = add(n.$field);)*
            };
        }

        field!(sysname, nodename, release, version, machine);
        assert!(u16::try_from(buf.len()).is_ok());
        Self { buf, sysname, nodename, release, version, machine }
    }

    fn gets(&self, r: &Range<u16>) -> &str {
        &self.buf[r.start as usize..r.end as usize]
    }

    pub(crate) fn sysname(&self) -> &str {
        self.gets(&self.sysname)
    }

    pub(crate) fn nodename(&self) -> &str {
        self.gets(&self.nodename)
    }

    pub(crate) fn release(&self) -> &str {
        self.gets(&self.release)
    }

    #[allow(dead_code)]
    pub(crate) fn version(&self) -> &str {
        self.gets(&self.version)
    }

    pub(crate) fn machine(&self) -> &str {
        self.gets(&self.machine)
    }
}

pub fn uname() -> Uname {
    Uname::new()
}
