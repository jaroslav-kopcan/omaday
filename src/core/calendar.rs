use chrono::{Datelike, Local, NaiveDate, Weekday};

/// English three letter month names, index 0 is January.
pub const MONTH_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// One day in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayEntry {
    pub date: NaiveDate,
    pub number: u32,
    pub weekday: &'static str,
    pub has_note: bool,
    pub is_today: bool,
}

/// The days of one month, ready for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthView {
    pub year: i32,
    pub month: u32,
    pub days: Vec<DayEntry>,
}

/// Today's date in the local time zone.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Number of days in the month. `month` is 1 to 12.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = next_month(year, month);
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .and_then(|first| first.pred_opt())
        .map(|last| last.day())
        .expect("month is 1 to 12 and the year is in range")
}

pub fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

pub fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

/// Full English weekday name.
pub fn weekday_name(date: NaiveDate) -> &'static str {
    match date.weekday() {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

impl MonthView {
    /// Build the list for a month. `days_with_notes` holds day numbers that have a file.
    pub fn build(year: i32, month: u32, today: NaiveDate, days_with_notes: &[u32]) -> MonthView {
        let days = (1..=days_in_month(year, month))
            .map(|number| {
                let date = NaiveDate::from_ymd_opt(year, month, number)
                    .expect("day number is within the month");
                DayEntry {
                    date,
                    number,
                    weekday: weekday_name(date),
                    has_note: days_with_notes.contains(&number),
                    is_today: date == today,
                }
            })
            .collect();
        MonthView { year, month, days }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn month_lengths() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn weekday_names_are_english_full_names() {
        assert_eq!(weekday_name(date(2026, 9, 28)), "Monday");
        assert_eq!(weekday_name(date(2026, 10, 4)), "Sunday");
        assert_eq!(weekday_name(date(2024, 2, 29)), "Thursday");
    }

    #[test]
    fn month_stepping_rolls_across_years() {
        assert_eq!(next_month(2026, 12), (2027, 1));
        assert_eq!(next_month(2026, 9), (2026, 10));
        assert_eq!(prev_month(2026, 1), (2025, 12));
        assert_eq!(prev_month(2026, 9), (2026, 8));
    }

    #[test]
    fn month_view_marks_notes_and_today() {
        let view = MonthView::build(2026, 9, date(2026, 9, 28), &[3, 28]);
        assert_eq!(view.year, 2026);
        assert_eq!(view.month, 9);
        assert_eq!(view.days.len(), 30);
        assert_eq!(
            view.days[0],
            DayEntry {
                date: date(2026, 9, 1),
                number: 1,
                weekday: "Tuesday",
                has_note: false,
                is_today: false,
            }
        );
        assert!(view.days[2].has_note);
        assert!(!view.days[2].is_today);
        assert!(view.days[27].has_note);
        assert!(view.days[27].is_today);
        assert_eq!(view.days[27].weekday, "Monday");
    }

    #[test]
    fn month_view_for_another_month_has_no_today() {
        let view = MonthView::build(2026, 8, date(2026, 9, 28), &[]);
        assert_eq!(view.days.len(), 31);
        assert!(view.days.iter().all(|d| !d.is_today && !d.has_note));
    }

    #[test]
    fn today_is_a_plausible_date() {
        assert!(today().year() >= 2026);
    }
}
