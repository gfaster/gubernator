pub mod machine;
pub mod memory;
pub mod protocol;
pub mod util;
pub mod job_req;
mod uname;
pub mod sel_expr;

pub use memory::Memory;
pub use protocol::*;

#[derive(Debug)]
pub struct Job {
    pub min_mem: Memory,
    pub threads: u16,
}
