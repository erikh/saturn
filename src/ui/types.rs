#[derive(Debug, Clone, Default)]
pub enum ListType {
    #[default]
    All,
    Today,
    Recurring,
}

#[derive(Debug, Clone)]
pub enum CommandType {
    Delete(Vec<u64>),
    DeleteRecurring(Vec<u64>),
    Entry(String),
    Edit(bool, u64),
}
