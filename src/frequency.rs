use rusqlite::params;
use std::fs;
use crate::db;

/// Column names for per-category frequency counts, in TSV order (cols 1, 4–18).
/// Exposed so callers can build weighted scores with user-configured bias.
pub const FREQ_COLS: &[&str] = &[
    "freq_total",
    "freq_howto", "freq_science", "freq_entertainment", "freq_education",
    "freq_people", "freq_music", "freq_autos", "freq_comedy", "freq_film",
    "freq_gaming", "freq_sports", "freq_news", "freq_nonprofits",
    "freq_travel", "freq_pets",
];

pub fn init_frequency(db_path: &str, freq_path: &str) {
    let conn = db::open(db_path);

    let already_done: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM words WHERE freq_total > 0",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if already_done {
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM words WHERE freq_total > 0", [], |r| r.get(0))
            .unwrap_or(0);
        println!("Frequency data already loaded ({} words matched), skipping.", n);
        return;
    }

    conn.execute_batch(
        "CREATE TEMP TABLE freq_data (
            word             TEXT PRIMARY KEY,
            freq_total       INTEGER NOT NULL DEFAULT 0,
            freq_howto       INTEGER NOT NULL DEFAULT 0,
            freq_science     INTEGER NOT NULL DEFAULT 0,
            freq_entertainment INTEGER NOT NULL DEFAULT 0,
            freq_education   INTEGER NOT NULL DEFAULT 0,
            freq_people      INTEGER NOT NULL DEFAULT 0,
            freq_music       INTEGER NOT NULL DEFAULT 0,
            freq_autos       INTEGER NOT NULL DEFAULT 0,
            freq_comedy      INTEGER NOT NULL DEFAULT 0,
            freq_film        INTEGER NOT NULL DEFAULT 0,
            freq_gaming      INTEGER NOT NULL DEFAULT 0,
            freq_sports      INTEGER NOT NULL DEFAULT 0,
            freq_news        INTEGER NOT NULL DEFAULT 0,
            freq_nonprofits  INTEGER NOT NULL DEFAULT 0,
            freq_travel      INTEGER NOT NULL DEFAULT 0,
            freq_pets        INTEGER NOT NULL DEFAULT 0
        )",
    )
    .expect("failed to create freq_data temp table");

    let content = fs::read_to_string(freq_path).expect("failed to read frequency file");
    println!("Loading frequency data...");

    conn.execute_batch("BEGIN").expect("begin");
    let mut stmt = conn
        .prepare(
            "INSERT OR IGNORE INTO freq_data VALUES \
             (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
        )
        .expect("failed to prepare freq insert");

    let mut count = 0u32;
    for line in content.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 19 {
            continue;
        }
        let p = |i: usize| -> i64 { cols[i].parse().unwrap_or(0) };
        stmt.execute(params![
            cols[0],
            p(1),  // total
            p(4),  // howto
            p(5),  // science
            p(6),  // entertainment
            p(7),  // education
            p(8),  // people
            p(9),  // music
            p(10), // autos
            p(11), // comedy
            p(12), // film
            p(13), // gaming
            p(14), // sports
            p(15), // news
            p(16), // nonprofits
            p(17), // travel
            p(18), // pets
        ])
        .expect("failed to insert freq entry");
        count += 1;
        if count % 50_000 == 0 {
            conn.execute_batch("COMMIT; BEGIN").expect("batch commit");
            println!("  {} entries...", count);
        }
    }
    conn.execute_batch("COMMIT").expect("final commit");
    println!("  {} frequency entries loaded", count);

    println!("Matching frequencies to dictionary...");
    conn.execute_batch(
        "UPDATE words
         SET
             freq_total         = f.freq_total,
             freq_howto         = f.freq_howto,
             freq_science       = f.freq_science,
             freq_entertainment = f.freq_entertainment,
             freq_education     = f.freq_education,
             freq_people        = f.freq_people,
             freq_music         = f.freq_music,
             freq_autos         = f.freq_autos,
             freq_comedy        = f.freq_comedy,
             freq_film          = f.freq_film,
             freq_gaming        = f.freq_gaming,
             freq_sports        = f.freq_sports,
             freq_news          = f.freq_news,
             freq_nonprofits    = f.freq_nonprofits,
             freq_travel        = f.freq_travel,
             freq_pets          = f.freq_pets
         FROM freq_data f
         WHERE words.word = f.word",
    )
    .expect("failed to update word frequencies");

    let matched: i64 = conn
        .query_row("SELECT COUNT(*) FROM words WHERE freq_total > 0", [], |r| r.get(0))
        .unwrap_or(0);
    println!("Frequency import complete: {} words matched in dictionary", matched);
}
