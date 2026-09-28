use chrono::{Datelike, NaiveDate};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// A day's file content and the modification time seen when it was read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub text: String,
    pub modified: SystemTime,
}

#[derive(Debug)]
pub enum VaultError {
    NotADirectory(PathBuf),
    Io { path: PathBuf, source: io::Error },
    NotUtf8(PathBuf),
    ConflictCopy { path: PathBuf, source: io::Error },
}

impl fmt::Display for VaultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VaultError::NotADirectory(p) => write!(f, "vault folder not found: {}", p.display()),
            VaultError::Io { path, source } => write!(f, "{}: {}", path.display(), source),
            VaultError::NotUtf8(p) => write!(f, "{} is not valid UTF-8", p.display()),
            VaultError::ConflictCopy { path, source } => write!(
                f,
                "could not keep a copy of the outside edit as {}: {}",
                path.display(),
                source
            ),
        }
    }
}

impl std::error::Error for VaultError {}

/// The folder holding `YYYY-MM-DD.md` files.
pub struct Vault {
    root: PathBuf,
}

/// `2026-09-28` becomes `2026-09-28.md`.
pub fn file_name_for(date: NaiveDate) -> String {
    format!("{}.md", date.format("%Y-%m-%d"))
}

/// Accept exactly `YYYY-MM-DD.md` with a real calendar date, nothing else.
pub fn parse_day_file_name(name: &str) -> Option<NaiveDate> {
    let stem = name.strip_suffix(".md")?;
    let bytes = stem.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let digit_positions = [0, 1, 2, 3, 5, 6, 8, 9];
    if !digit_positions.iter().all(|&i| bytes[i].is_ascii_digit()) {
        return None;
    }
    let year: i32 = stem[0..4].parse().ok()?;
    let month: u32 = stem[5..7].parse().ok()?;
    let day: u32 = stem[8..10].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> VaultError {
    let path = path.to_path_buf();
    move |source| VaultError::Io { path, source }
}

impl Vault {
    pub fn open(root: PathBuf) -> Result<Vault, VaultError> {
        if root.is_dir() {
            Ok(Vault { root })
        } else {
            Err(VaultError::NotADirectory(root))
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path_for(&self, date: NaiveDate) -> PathBuf {
        self.root.join(file_name_for(date))
    }

    /// Dates of day files larger than zero bytes, sorted ascending.
    pub fn note_dates(&self) -> Result<Vec<NaiveDate>, VaultError> {
        let entries = fs::read_dir(&self.root).map_err(io_error(&self.root))?;
        let mut dates = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error(&self.root))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(date) = parse_day_file_name(name) else {
                continue;
            };
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_file() && meta.len() > 0 {
                dates.push(date);
            }
        }
        dates.sort_unstable();
        Ok(dates)
    }

    /// Day numbers within one month that have a note.
    pub fn days_with_notes(&self, year: i32, month: u32) -> Result<Vec<u32>, VaultError> {
        Ok(self
            .note_dates()?
            .into_iter()
            .filter(|d| d.year() == year && d.month() == month)
            .map(|d| d.day())
            .collect())
    }

    /// Distinct years that have at least one note, ascending.
    pub fn years_with_notes(&self) -> Result<Vec<i32>, VaultError> {
        let mut years: Vec<i32> = self.note_dates()?.into_iter().map(|d| d.year()).collect();
        years.dedup();
        Ok(years)
    }

    /// The day's file, or `None` when it does not exist.
    pub fn read(&self, date: NaiveDate) -> Result<Option<Note>, VaultError> {
        let path = self.path_for(date);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(VaultError::Io { path, source }),
        };
        let modified = fs::metadata(&path)
            .and_then(|m| m.modified())
            .map_err(io_error(&path))?;
        let text = String::from_utf8(bytes).map_err(|_| VaultError::NotUtf8(path))?;
        Ok(Some(Note { text, modified }))
    }

    /// Write the text with temp file, fsync and rename. `expected` is the
    /// modification time last seen; a different time on disk means an
    /// outside edit, which is copied to the conflicts folder first.
    pub fn write(
        &self,
        date: NaiveDate,
        text: &str,
        expected: Option<SystemTime>,
    ) -> Result<SystemTime, VaultError> {
        let target = self.path_for(date);
        self.guard_outside_change(date, &target, expected)?;
        let tmp = self
            .root
            .join(format!(".{}.omaday-tmp", file_name_for(date)));
        {
            let mut file = File::create(&tmp).map_err(io_error(&tmp))?;
            file.write_all(text.as_bytes()).map_err(io_error(&tmp))?;
            file.sync_all().map_err(io_error(&tmp))?;
        }
        fs::rename(&tmp, &target).map_err(io_error(&target))?;
        fs::metadata(&target)
            .and_then(|m| m.modified())
            .map_err(io_error(&target))
    }

    /// Remove the day's file. A missing file is not an error.
    pub fn delete(&self, date: NaiveDate, expected: Option<SystemTime>) -> Result<(), VaultError> {
        let target = self.path_for(date);
        self.guard_outside_change(date, &target, expected)?;
        match fs::remove_file(&target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(VaultError::Io {
                path: target,
                source,
            }),
        }
    }

    fn guard_outside_change(
        &self,
        date: NaiveDate,
        target: &Path,
        expected: Option<SystemTime>,
    ) -> Result<(), VaultError> {
        let current = match fs::metadata(target) {
            Ok(meta) => meta.modified().map_err(io_error(target))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(VaultError::Io {
                    path: target.to_path_buf(),
                    source,
                });
            }
        };
        if expected == Some(current) {
            return Ok(());
        }
        self.keep_conflict_copy(date, target)
    }

    fn keep_conflict_copy(&self, date: NaiveDate, target: &Path) -> Result<(), VaultError> {
        let folder = self.root.join(".omaday").join("conflicts");
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let copy = folder.join(format!("{}.{}.md", date.format("%Y-%m-%d"), seconds));
        fs::create_dir_all(&folder)
            .and_then(|_| fs::copy(target, &copy))
            .map(|_| ())
            .map_err(|source| VaultError::ConflictCopy { path: copy, source })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn vault_in(dir: &tempfile::TempDir) -> Vault {
        Vault::open(dir.path().to_path_buf()).unwrap()
    }

    #[test]
    fn day_file_names_round_trip() {
        assert_eq!(file_name_for(date(2026, 9, 28)), "2026-09-28.md");
        assert_eq!(
            parse_day_file_name("2026-09-28.md"),
            Some(date(2026, 9, 28))
        );
        assert_eq!(
            parse_day_file_name("2024-02-29.md"),
            Some(date(2024, 2, 29))
        );
    }

    #[test]
    fn other_names_are_rejected() {
        for name in [
            "2026-09-28",
            "2026-09-28.txt",
            "2026-9-28.md",
            "26-09-28.md",
            "2026-02-30.md",
            "notes.md",
            "2026-09-28x.md",
            "abcd-ef-gh.md",
            ".2026-09-28.md.omaday-tmp",
        ] {
            assert_eq!(parse_day_file_name(name), None, "{name}");
        }
    }

    #[test]
    fn open_requires_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Vault::open(dir.path().to_path_buf()).is_ok());
        let missing = dir.path().join("missing");
        assert!(
            matches!(Vault::open(missing.clone()), Err(VaultError::NotADirectory(p)) if p == missing)
        );
    }

    #[test]
    fn path_for_joins_root_and_name() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        assert_eq!(vault.root(), dir.path());
        assert_eq!(
            vault.path_for(date(2026, 9, 28)),
            dir.path().join("2026-09-28.md")
        );
    }

    #[test]
    fn listing_ignores_other_and_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("2026-09-28.md"), "x").unwrap();
        fs::write(dir.path().join("2026-09-03.md"), "y").unwrap();
        fs::write(dir.path().join("2025-12-31.md"), "z").unwrap();
        fs::write(dir.path().join("2026-09-10.md"), "").unwrap();
        fs::write(dir.path().join("readme.md"), "no").unwrap();
        fs::create_dir(dir.path().join("2026-09-11.md")).unwrap();
        let vault = vault_in(&dir);
        assert_eq!(
            vault.note_dates().unwrap(),
            vec![date(2025, 12, 31), date(2026, 9, 3), date(2026, 9, 28)]
        );
        assert_eq!(vault.days_with_notes(2026, 9).unwrap(), vec![3, 28]);
        assert_eq!(vault.days_with_notes(2026, 8).unwrap(), Vec::<u32>::new());
        assert_eq!(vault.years_with_notes().unwrap(), vec![2025, 2026]);
    }

    #[test]
    fn listing_an_unreadable_root_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        drop(dir);
        assert!(matches!(vault.note_dates(), Err(VaultError::Io { .. })));
    }

    #[test]
    fn read_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        assert_eq!(vault.read(date(2026, 9, 28)).unwrap(), None);
    }

    #[test]
    fn write_then_read_round_trips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        let modified = vault.write(d, "hello\n", None).unwrap();
        let note = vault.read(d).unwrap().unwrap();
        assert_eq!(note.text, "hello\n");
        assert_eq!(note.modified, modified);
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["2026-09-28.md".to_string()]);
    }

    #[test]
    fn delete_removes_the_file_and_tolerates_a_missing_one() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        let modified = vault.write(d, "x", None).unwrap();
        vault.delete(d, Some(modified)).unwrap();
        assert!(!vault.path_for(d).exists());
        vault.delete(d, None).unwrap();
    }

    #[test]
    fn invalid_utf8_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        fs::write(vault.path_for(d), [0xff, 0xfe, b'a']).unwrap();
        assert!(matches!(vault.read(d), Err(VaultError::NotUtf8(_))));
    }

    #[test]
    fn matching_time_writes_without_a_conflict_copy() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        let t1 = vault.write(d, "one", None).unwrap();
        vault.write(d, "two", Some(t1)).unwrap();
        assert!(!dir.path().join(".omaday").exists());
        assert_eq!(vault.read(d).unwrap().unwrap().text, "two");
    }

    fn change_outside(path: &Path, after: SystemTime) {
        fs::write(path, "outside").unwrap();
        File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(after + Duration::from_secs(5))
            .unwrap();
    }

    fn conflict_copies(dir: &tempfile::TempDir) -> Vec<PathBuf> {
        let folder = dir.path().join(".omaday").join("conflicts");
        match fs::read_dir(&folder) {
            Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
            Err(_) => Vec::new(),
        }
    }

    #[test]
    fn outside_change_is_copied_before_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        let t1 = vault.write(d, "mine v1", None).unwrap();
        let path = vault.path_for(d);
        change_outside(&path, t1);
        vault.write(d, "mine v2", Some(t1)).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "mine v2");
        let copies = conflict_copies(&dir);
        assert_eq!(copies.len(), 1);
        let name = copies[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            name.starts_with("2026-09-28.") && name.ends_with(".md"),
            "{name}"
        );
        assert_eq!(fs::read_to_string(&copies[0]).unwrap(), "outside");
    }

    #[test]
    fn file_created_outside_is_copied_too() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        fs::write(vault.path_for(d), "outside").unwrap();
        vault.write(d, "mine", None).unwrap();
        assert_eq!(conflict_copies(&dir).len(), 1);
        assert_eq!(fs::read_to_string(vault.path_for(d)).unwrap(), "mine");
    }

    #[test]
    fn outside_change_before_delete_is_copied() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        let t1 = vault.write(d, "mine", None).unwrap();
        change_outside(&vault.path_for(d), t1);
        vault.delete(d, Some(t1)).unwrap();
        assert!(!vault.path_for(d).exists());
        assert_eq!(conflict_copies(&dir).len(), 1);
    }

    #[test]
    fn failed_conflict_copy_blocks_the_write() {
        let dir = tempfile::tempdir().unwrap();
        let vault = vault_in(&dir);
        let d = date(2026, 9, 28);
        let t1 = vault.write(d, "mine v1", None).unwrap();
        let path = vault.path_for(d);
        change_outside(&path, t1);
        fs::write(dir.path().join(".omaday"), "not a folder").unwrap();
        assert!(matches!(
            vault.write(d, "mine v2", Some(t1)),
            Err(VaultError::ConflictCopy { .. })
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "outside");
    }
}
