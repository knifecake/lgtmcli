use super::TableOutput;

pub fn render<T: TableOutput>(value: &T) {
    value.render_table();
}

pub fn render_aligned(headers: &[&str], rows: &[Vec<String>]) {
    print!("{}", format_aligned(headers, rows));
}

fn format_aligned(headers: &[&str], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }

    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();

    for row in rows {
        for (idx, cell) in row.iter().enumerate().take(widths.len()) {
            widths[idx] = widths[idx].max(cell.chars().count());
        }
    }

    let mut output = String::new();
    write_row(
        &mut output,
        headers.iter().map(|h| h.to_string()).collect(),
        &widths,
    );
    for row in rows {
        let mut padded = row.clone();
        if padded.len() < widths.len() {
            padded.resize(widths.len(), String::new());
        }
        write_row(&mut output, padded, &widths);
    }

    output
}

fn write_row(output: &mut String, cells: Vec<String>, widths: &[usize]) {
    for (idx, width) in widths.iter().enumerate() {
        let cell = cells.get(idx).map(String::as_str).unwrap_or("");
        output.push_str(&format!("{cell:<width$}", width = *width));
        if idx + 1 < widths.len() {
            output.push_str("  ");
        }
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::format_aligned;

    #[test]
    fn aligned_output_keeps_columns_consistent() {
        let headers = ["ID", "NAME", "TYPE"];
        let rows = vec![
            vec!["1".to_string(), "A".to_string(), "loki".to_string()],
            vec![
                "22".to_string(),
                "VeryLongName".to_string(),
                "prometheus".to_string(),
            ],
        ];

        let output = format_aligned(&headers, &rows);
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), 3);

        let name_col = lines[0].find("NAME").expect("NAME header present");
        let type_col = lines[0].find("TYPE").expect("TYPE header present");

        assert_eq!(lines[1].find('A'), Some(name_col));
        assert_eq!(lines[1].find("loki"), Some(type_col));
        assert_eq!(lines[2].find("VeryLongName"), Some(name_col));
        assert_eq!(lines[2].find("prometheus"), Some(type_col));
    }

    #[test]
    fn empty_headers_produce_empty_output() {
        let output = format_aligned(&[], &[]);
        assert!(output.is_empty());
    }
}
