
pub const PAGE_TABLE_OFFSET: u64 = size_tb!(10); // 10 TB
pub const PROCESS_STACK_ADDRESS: usize = size_gb!(5); // 5GB

pub const HEAP_START: usize = size_tb!(3);
pub const HEAP_SIZE: usize = size_mb!(10);

pub const KERNEL_CODE: u64 = size_tb!(2);