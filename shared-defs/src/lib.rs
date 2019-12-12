#[repr(u64)]
#[derive(Debug)]
pub enum Tag {
    Ready = 1,
    Pending = 2,
}

#[repr(C)]
#[derive(Debug)]
pub struct PollResult {
    pub tag: Tag,
    pub result: u64,
}
