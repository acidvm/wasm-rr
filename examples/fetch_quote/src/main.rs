fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Actually make an HTTP request using wasi-http-client
    let mut url = "https://bash-org-archive.com/?random1".to_string();
    println!("Fetching quote from {}...", url);

    // Perform the actual HTTP request using wasi-http-client
    let client = wasi_http_client::Client::new();
    let mut response = client.get(&url).send()?;

    // Handle redirects manually (wasi-http-client doesn't follow them automatically)
    let mut redirect_count = 0;
    while response.status() >= 300 && response.status() < 400 && redirect_count < 5 {
        // Get the Location header for redirect
        let location = response
            .headers()
            .get("location")
            .ok_or("Redirect response missing Location header")?;

        println!("Following redirect to: {}", location);

        // Make the URL absolute if it's relative
        url = if location.starts_with("http://") || location.starts_with("https://") {
            location.clone()
        } else if location.starts_with("/") {
            format!("https://bash-org-archive.com{}", location)
        } else {
            format!("https://bash-org-archive.com/{}", location)
        };

        response = client.get(&url).send()?;
        redirect_count += 1;
    }

    let status = response.status();
    if status != 200 {
        return Err(format!("HTTP request failed with status: {}", status).into());
    }

    let html = String::from_utf8(response.body()?)?;

    // Extract the quote from the HTML
    let quote = extract_quote(&html)?;

    println!("\n--- Quote #22754 ---");
    println!("{}", quote);
    println!("-------------------\n");

    Ok(())
}

fn extract_quote(html: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Find the quote content between <p class="qt"> and </p>
    let start_marker = r#"<p class="qt">"#;
    let end_marker = "</p>";

    let start_pos = html
        .find(start_marker)
        .ok_or("Could not find quote start marker")?;

    let quote_start = start_pos + start_marker.len();
    let remaining = &html[quote_start..];

    let end_pos = remaining
        .find(end_marker)
        .ok_or("Could not find quote end marker")?;

    let raw_quote = &remaining[..end_pos];

    // Clean up HTML entities and formatting
    let quote = raw_quote
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("<br />", "\n")
        .trim()
        .to_string();

    if quote.is_empty() {
        return Err("Extracted quote is empty".into());
    }

    Ok(quote)
}
