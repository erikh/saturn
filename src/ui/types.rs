#[derive(Debug, Clone, Default)]
pub enum ListType {
    #[default]
    All,
    Today,
    Recurring,
    Search,
}

#[derive(Debug, Clone)]
pub enum CommandType {
    Delete(Vec<u64>),
    DeleteRecurring(Vec<u64>),
    Entry(String),
    Edit(bool, u64),
    Show(bool, u64),
    Search(Vec<String>),
}
