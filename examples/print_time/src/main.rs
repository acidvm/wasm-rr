fn main() {
    let now = time::OffsetDateTime::now_utc();
    // RFC3339 format, e.g., 2025-09-28T12:34:56Z
    println!(
        "{}",
        now.format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    );
}
