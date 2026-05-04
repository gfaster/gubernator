//! shared helpful utilities

use std::fmt;

pub struct Truncate<'a, 'b> {
    underlying: &'a mut fmt::Formatter<'b>,
    rem: usize,
    escape_queued: bool,
    quote_queued: bool,
}

impl<'a, 'b> Truncate<'a, 'b> {
    pub fn new(underlying: &'a mut fmt::Formatter<'b>, len: usize) -> Self {
        Truncate { underlying, rem: len, escape_queued: false, quote_queued: false }
    }
}

impl fmt::Write for Truncate<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // TODO: make this a little more efficient
        const EXTRA: usize = 2;

        let Some(rem) = self.rem.checked_sub(EXTRA) else {
            return Ok(())
        };

        let (consumed, pos) = s.char_indices().enumerate().take(rem).map(|(i, (pos, c))| {
            let ret = (i + 1, pos + c.len_utf8());
            if self.escape_queued {
                self.escape_queued = false;
                return ret
            }
            if c == '\\' {
                self.escape_queued = true
            }
            if c == '"' {
                self.quote_queued = !self.quote_queued
            }
            ret
        }).last().unwrap_or_default();

        self.rem = rem + EXTRA - consumed;

        self.underlying.write_str(&s[..pos])?;

        if self.rem > EXTRA {
            // still more room
            return Ok(())
        }

        // we're truncating here
        self.rem = 0;

        if self.escape_queued {
            if let Some(escaped) = s.get(pos..).and_then(|s| s.chars().next()) {
                self.underlying.write_char(escaped)?;
            }
        }

        if self.quote_queued {
            self.underlying.write_char('"')?;
        }

        self.underlying.write_str("...")
    }
}


pub fn truncate_str_debug(s: &str, len: usize) -> impl fmt::Debug + fmt::Display {
    fmt::from_fn(move |f| {
        use fmt::Write;
        let mut t = Truncate::new(f, len);
        write!(t, "{s:?}")
    })
}
