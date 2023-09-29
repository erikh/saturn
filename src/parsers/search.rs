use super::time::{parse_date, parse_time};
use crate::record::Record;
use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct SearchParser {
    args: Vec<String>,
    to_search: Vec<Record>,
}

impl SearchParser {
    pub fn new(args: Vec<String>, to_search: Vec<Record>) -> Self {
        Self { args, to_search }
    }

    pub fn perform(&self) -> Result<Vec<Record>> {
        filter(self.to_search.clone(), parse_search(self.args.clone())?)
    }
}

pub enum SearchParserState {
    Field,
    FieldKey,
    FieldKeyValue,
    FieldValue,
    FieldValueValue,
    Date,
    FromDate,
    FromDateStartValue,
    FromDateEndValue,
    Time,
    FromTime,
    FromTimeStartValue,
    FromTimeEndValue,
    Detail,
    FromRecur,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SearchParserAction {
    Field(String, Option<String>),
    Date(chrono::NaiveDate),
    FromDate(chrono::NaiveDate, chrono::NaiveDate),
    Time(chrono::NaiveTime),
    FromTime(chrono::NaiveTime, chrono::NaiveTime),
    Detail(String),
    Recur(u64),
    Done(bool),
}

fn parse_search(args: Vec<String>) -> Result<Vec<SearchParserAction>> {
    let mut state: Option<SearchParserState> = None;
    let mut kept: Option<String> = None;
    let mut actions = Vec::new();

    for cmd in args {
        if let Some(ref inner) = state {
            match inner {
                SearchParserState::Field => match cmd.as_str() {
                    "key" => state = Some(SearchParserState::FieldKey),
                    "value" => state = Some(SearchParserState::FieldValue),
                    _ => return Err(anyhow!("Invalid syntax")),
                },
                SearchParserState::FieldKey => {
                    if kept.is_some() {
                        actions.push(SearchParserAction::Field(cmd, kept.clone()));
                        kept = None;
                        state = None;
                    } else {
                        kept = Some(cmd);
                        state = Some(SearchParserState::FieldKeyValue)
                    }
                }
                SearchParserState::FieldKeyValue => match cmd.as_str() {
                    "value" => state = Some(SearchParserState::FieldValue),
                    _ => {
                        actions.push(SearchParserAction::Field(kept.unwrap(), None));
                        kept = None;
                        state = None;
                    }
                },
                SearchParserState::FieldValue => {
                    if let Some(ref kept) = kept {
                        actions.push(SearchParserAction::Field(kept.clone(), Some(cmd)));
                        state = None;
                    } else {
                        kept = Some(cmd);
                        state = Some(SearchParserState::FieldValueValue);
                    }
                }
                SearchParserState::FieldValueValue => match cmd.as_str() {
                    "key" => state = Some(SearchParserState::FieldKey),
                    _ => return Err(anyhow!("Cannot search fields without a key")),
                },
                SearchParserState::Date => match cmd.as_str() {
                    "from" => state = Some(SearchParserState::FromDate),
                    _ => {
                        actions.push(SearchParserAction::Date(parse_date(cmd)?));
                        state = None;
                    }
                },
                SearchParserState::FromDate => {
                    kept = Some(cmd);
                    state = Some(SearchParserState::FromDateStartValue);
                }
                SearchParserState::FromDateStartValue => match cmd.as_str() {
                    "to" => state = Some(SearchParserState::FromDateEndValue),
                    _ => return Err(anyhow!("syntax: from <date> to <date>")),
                },
                SearchParserState::FromDateEndValue => {
                    actions.push(SearchParserAction::FromDate(
                        parse_date(kept.clone().unwrap())?,
                        parse_date(cmd)?,
                    ));
                    kept = None;
                    state = None;
                }
                SearchParserState::Time => match cmd.as_str() {
                    "from" => state = Some(SearchParserState::FromTime),
                    _ => {
                        actions.push(SearchParserAction::Time(parse_time(cmd, false)?));
                        state = None;
                    }
                },
                SearchParserState::FromTime => {
                    kept = Some(cmd);
                    state = Some(SearchParserState::FromTimeStartValue);
                }
                SearchParserState::FromTimeStartValue => match cmd.as_str() {
                    "to" => state = Some(SearchParserState::FromTimeEndValue),
                    _ => return Err(anyhow!("syntax: from <time> to <time>")),
                },
                SearchParserState::FromTimeEndValue => {
                    actions.push(SearchParserAction::FromTime(
                        parse_time(kept.clone().unwrap(), false)?,
                        parse_time(cmd, false)?,
                    ));
                    kept = None;
                    state = None;
                }
                SearchParserState::Detail => {
                    actions.push(SearchParserAction::Detail(cmd));
                    kept = None;
                    state = None;
                }
                SearchParserState::FromRecur => {
                    actions.push(SearchParserAction::Recur(cmd.parse()?));
                    kept = None;
                    state = None;
                }
            }
        } else {
            match cmd.as_str() {
                "field" => state = Some(SearchParserState::Field),
                "date" => state = Some(SearchParserState::Date),
                "time" => state = Some(SearchParserState::Time),
                "detail" => state = Some(SearchParserState::Detail),
                "recur" => state = Some(SearchParserState::FromRecur),
                "finished" => actions.push(SearchParserAction::Done(true)),
                "unfinished" => actions.push(SearchParserAction::Done(false)),
                _ => return Err(anyhow!("Invalid syntax")),
            }
        }
    }

    // clean up trailing state
    if state.is_some() && kept.is_some() {
        match state.unwrap() {
            SearchParserState::FieldKeyValue => {
                actions.push(SearchParserAction::Field(kept.unwrap(), None))
            }
            _ => {}
        }
    }

    if actions.is_empty() {
        Err(anyhow!("Invalid syntax"))
    } else {
        Ok(actions)
    }
}

fn filter(to_search: Vec<Record>, actions: Vec<SearchParserAction>) -> Result<Vec<Record>> {
    let mut ret = Vec::new();
    'items: for item in &to_search {
        for action in &actions {
            let matched = match action {
                SearchParserAction::Field(key, value) => {
                    let fields = item.fields();
                    let ret = fields.iter().find(|(k, v)| {
                        if *k == key {
                            if let Some(value) = value {
                                for val in *v {
                                    if *value == *val {
                                        return true;
                                    }
                                }

                                return false;
                            } else {
                                return true;
                            }
                        } else {
                            return false;
                        }
                    });

                    ret.is_some()
                }

                SearchParserAction::Date(date) => item.date() == *date,
                SearchParserAction::FromDate(start, end) => {
                    let itemdate = item.date();
                    itemdate >= *start && itemdate <= *end
                }
                SearchParserAction::Time(time) => item.datetime().time() == *time,
                SearchParserAction::FromTime(start, end) => {
                    let itemdate = item.datetime().time();
                    itemdate >= *start && itemdate <= *end
                }
                SearchParserAction::Detail(message) => item
                    .detail()
                    .to_lowercase()
                    .contains(&message.to_lowercase()),
                SearchParserAction::Recur(id) => {
                    if let Some(key) = item.recurrence_key() {
                        key == *id
                    } else {
                        false
                    }
                }
                SearchParserAction::Done(done) => item.completed() == *done,
            };

            if !matched {
                continue 'items;
            }
        }

        ret.push(item.clone());
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_search_parser() {
        use super::{parse_search, SearchParserAction};

        let table = vec![
            (
                "time 8pm",
                vec![SearchParserAction::Time(
                    chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap(),
                )],
            ),
            (
                "date today",
                vec![SearchParserAction::Date(chrono::Local::now().date_naive())],
            ),
            (
                "date today time 8pm",
                vec![
                    SearchParserAction::Date(chrono::Local::now().date_naive()),
                    SearchParserAction::Time(chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
                ],
            ),
            (
                "date today time 8pm finished",
                vec![
                    SearchParserAction::Date(chrono::Local::now().date_naive()),
                    SearchParserAction::Time(chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
                    SearchParserAction::Done(true),
                ],
            ),
            (
                "date today time 8pm unfinished detail foobar recur 12",
                vec![
                    SearchParserAction::Date(chrono::Local::now().date_naive()),
                    SearchParserAction::Time(chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()),
                    SearchParserAction::Done(false),
                    SearchParserAction::Detail("foobar".to_string()),
                    SearchParserAction::Recur(12),
                ],
            ),
            (
                "date from today to tomorrow time from 8pm to 11pm unfinished detail foobar recur 12",
                vec![
                    SearchParserAction::FromDate(
                        chrono::Local::now().date_naive(),
                        (chrono::Local::now() + chrono::Duration::days(1)).date_naive(),
                    ),
                    SearchParserAction::FromTime(
                        chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap(),
                        chrono::NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
                    ),
                    SearchParserAction::Done(false),
                    SearchParserAction::Detail("foobar".to_string()),
                    SearchParserAction::Recur(12),
                ],
            ),
            (
                "field key foo value bar",
                vec![SearchParserAction::Field(
                    "foo".to_string(),
                    Some("bar".to_string()),
                )],
            ),
            (
                "field value foo key bar",
                vec![SearchParserAction::Field(
                    "bar".to_string(),
                    Some("foo".to_string()),
                )],
            ),
            (
                "field key bar",
                vec![SearchParserAction::Field("bar".to_string(), None)],
            ),
        ];

        for item in table {
            assert_eq!(
                parse_search(
                    item.0
                        .to_string()
                        .split(" ")
                        .map(ToString::to_string)
                        .collect()
                )
                .unwrap(),
                item.1,
                "{}",
                item.0,
            )
        }
    }

    #[test]
    fn test_filter() {
        use super::{filter, SearchParserAction};
        use crate::record::Record;
        use chrono::{NaiveDate, NaiveTime};

        enum Modification {
            Date(i32, u32, u32),
            Time(u32, u32, u32),
            Detail(String),
            RecurrenceKey(u64),
            Done(bool),
        }

        let table: Vec<(
            // index and modification to make (will be applied at test time)
            Vec<(usize, Modification)>,
            // List of actions to apply to the filter
            Vec<SearchParserAction>,
            // count of results the filter found
            usize,
            // friendly message for test failures
            &str,
        )> = vec![
            (
                vec![
                    (0, Modification::Date(2018, 10, 23)),
                    (1, Modification::Date(2020, 10, 23)),
                ],
                vec![SearchParserAction::Date(
                    chrono::NaiveDate::from_ymd_opt(2018, 10, 23).unwrap(),
                )],
                1,
                "static date",
            ),
            (
                vec![
                    (0, Modification::Time(16, 20, 0)),
                    (1, Modification::Time(16, 20, 0)),
                    (2, Modification::Time(4, 20, 0)),
                ],
                vec![SearchParserAction::Time(
                    chrono::NaiveTime::from_hms_opt(16, 20, 0).unwrap(),
                )],
                2,
                "static time",
            ),
            (
                vec![
                    (0, Modification::Date(2018, 10, 23)),
                    (1, Modification::Date(2020, 10, 23)),
                    (2, Modification::Date(2021, 10, 23)),
                ],
                vec![SearchParserAction::FromDate(
                    chrono::NaiveDate::from_ymd_opt(2018, 10, 23).unwrap(),
                    chrono::NaiveDate::from_ymd_opt(2020, 10, 23).unwrap(),
                )],
                2,
                "date range",
            ),
            (
                vec![
                    (0, Modification::Time(16, 20, 0)),
                    (1, Modification::Time(16, 20, 0)),
                    (2, Modification::Time(16, 30, 0)),
                    (3, Modification::Time(4, 20, 0)),
                ],
                vec![SearchParserAction::FromTime(
                    chrono::NaiveTime::from_hms_opt(16, 20, 0).unwrap(),
                    chrono::NaiveTime::from_hms_opt(16, 30, 0).unwrap(),
                )],
                3,
                "time range",
            ),
            (
                vec![
                    (0, Modification::Detail("foo".to_string())),
                    (1, Modification::Detail("poo".to_string())),
                    (2, Modification::Detail("stool".to_string())),
                    (3, Modification::Detail("bar".to_string())),
                    (4, Modification::Detail("baz".to_string())),
                ],
                vec![SearchParserAction::Detail("oo".to_string())],
                3,
                "detail substring",
            ),
            (
                vec![
                    (0, Modification::RecurrenceKey(1)),
                    (1, Modification::RecurrenceKey(2)),
                    (2, Modification::RecurrenceKey(3)),
                    (3, Modification::RecurrenceKey(2)),
                    (4, Modification::RecurrenceKey(4)),
                    (5, Modification::RecurrenceKey(2)),
                    (6, Modification::RecurrenceKey(5)),
                    (7, Modification::RecurrenceKey(6)),
                    (8, Modification::RecurrenceKey(1)),
                    (9, Modification::RecurrenceKey(3)),
                ],
                vec![SearchParserAction::Recur(2)],
                3,
                "recurrence id",
            ),
            (
                vec![
                    (0, Modification::Done(true)),
                    (1, Modification::Done(false)),
                    (2, Modification::Done(false)),
                    (3, Modification::Done(true)),
                    (4, Modification::Done(true)),
                    (5, Modification::Done(true)),
                    (6, Modification::Done(false)),
                    (7, Modification::Done(true)),
                    (8, Modification::Done(true)),
                    (9, Modification::Done(false)),
                ],
                vec![SearchParserAction::Done(true)],
                6,
                "done",
            ),
        ];

        for rules in table {
            let mut records = vec![
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
                Record::default(),
            ];

            for rule in rules.0 {
                match rule.1 {
                    Modification::Date(y, m, d) => {
                        records[rule.0].set_date(NaiveDate::from_ymd_opt(y, m, d).unwrap());
                    }
                    Modification::Time(h, m, s) => {
                        records[rule.0].set_at(NaiveTime::from_hms_opt(h, m, s));
                    }
                    Modification::Detail(detail) => {
                        records[rule.0].set_detail(detail);
                    }
                    Modification::RecurrenceKey(id) => {
                        records[rule.0].set_recurrence_key(Some(id));
                    }
                    Modification::Done(done) => {
                        records[rule.0].set_completed(done);
                    }
                }
            }

            let res = filter(records, rules.1).unwrap();
            assert_eq!(res.len(), rules.2, "test: {}", rules.3);
        }
    }
}
