#[repr(u64)]
pub enum Tag {
    Ready = 1,
    Pending = 2,
}

#[repr(C)]
pub struct PollResult {
    pub tag: Tag,
    pub result: u64,
}
