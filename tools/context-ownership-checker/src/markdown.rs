//! Deliberately bounded ATX-section profile described by proof-contract.json.
//! This is a structural scanner, not a general Markdown or meaning evaluator.

#[derive(Default, Debug)]
pub struct Section {
    pub h2_count: usize,
    pub other_top_level: bool,
    pub empty_title: bool,
    pub before_heading: bool,
    pub prose: bool,
    pub unclosed_block: bool,
    pub unsupported_html: bool,
    pub setext: bool,
    pub reference_definition: bool,
}

fn without_comments(line: &str, comment: &mut bool) -> String {
    let mut rest = line;
    let mut output = String::new();
    loop {
        if *comment {
            let Some(end) = rest.find("-->") else { break };
            rest = &rest[end + 3..];
            *comment = false;
        } else if let Some(start) = rest.find('`')
            && rest
                .find("<!--")
                .is_none_or(|comment_at| start < comment_at)
        {
            output.push_str(&rest[..start]);
            let code = &rest[start..];
            let length = code.bytes().take_while(|&b| b == b'`').count();
            if let Some(end) = inline_code_end(code, length) {
                output.push_str(&code[..end]);
                rest = &code[end..];
            } else {
                output.push_str(code);
                break;
            }
        } else if let Some(start) = rest.find("<!--") {
            output.push_str(&rest[..start]);
            rest = &rest[start + 4..];
            *comment = true;
        } else {
            output.push_str(rest);
            break;
        }
    }
    output
}

fn inline_code_end(code: &str, length: usize) -> Option<usize> {
    let bytes = code.as_bytes();
    let mut at = length;
    while at < bytes.len() {
        if bytes[at] == b'`' {
            let count = bytes[at..].iter().take_while(|&&b| b == b'`').count();
            if count == length {
                return Some(at + count);
            }
            at += count;
        } else {
            at += 1;
        }
    }
    None
}

fn has_prose(line: &str) -> bool {
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        if rest[..start].chars().any(char::is_alphanumeric) {
            return true;
        }
        let code = &rest[start..];
        let length = code.bytes().take_while(|&b| b == b'`').count();
        let Some(end) = inline_code_end(code, length) else {
            return code.chars().any(char::is_alphanumeric);
        };
        rest = &code[end..];
    }
    rest.chars().any(char::is_alphanumeric)
}

fn marker(line: &str) -> Option<(u8, usize, &str)> {
    let first = *line.as_bytes().first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let count = line.bytes().take_while(|&b| b == first).count();
    (count >= 3).then_some((first, count, &line[count..]))
}

pub fn inspect(text: &str) -> Section {
    let mut value = Section::default();
    let mut comment = false;
    let mut fence: Option<(u8, usize)> = None;
    let mut previous_paragraph = false;
    let lines: Vec<_> = text.lines().collect();
    for (line_index, &original_line) in lines.iter().enumerate() {
        // Comments inside code are literal; they must not hide the closing fence.
        if let Some((kind, length)) = fence {
            let line = original_line.trim_start_matches(' ');
            if original_line.len() - line.len() <= 3
                && let Some((actual, count, rest)) = marker(line)
                && actual == kind
                && count >= length
                && rest.trim().is_empty()
            {
                fence = None;
            }
            continue;
        }
        // Four spaces or a leading tab are an indented code block in this profile.
        // They never supply a visible section heading or its prose.
        if !comment && (original_line.starts_with("    ") || original_line.starts_with('\t')) {
            if value.h2_count == 0 && !original_line.trim().is_empty() {
                value.before_heading = true;
            }
            previous_paragraph = false;
            continue;
        }
        if !comment {
            let line = original_line.trim_start_matches(' ');
            if original_line.len() - line.len() <= 3
                && let Some((kind, count, _)) = marker(line)
            {
                if value.h2_count == 0 {
                    value.before_heading = true;
                }
                fence = Some((kind, count));
                previous_paragraph = false;
                continue;
            }
        }
        // A line-start HTML comment is a block through its closing line.
        // Removing it must not manufacture an ATX marker or fence on that line.
        let comment_block = comment || original_line.trim_start_matches(' ').starts_with("<!--");
        let visible = without_comments(original_line, &mut comment);
        if comment_block {
            previous_paragraph = false;
            continue;
        }
        let line = visible.trim_start_matches(' ');
        if visible.len() - line.len() > 3 {
            previous_paragraph = false;
            continue;
        }
        let line = line.trim_end();
        if line.is_empty() {
            previous_paragraph = false;
            continue;
        }
        // Raw HTML blocks/headings are explicitly outside this ATX-text profile.
        // HTML comments are supported above; no hidden HTML heading is counted.
        if line.starts_with('<') {
            value.unsupported_html = true;
            previous_paragraph = false;
            continue;
        }
        let original_marker = original_line.trim_start_matches(' ');
        let hashes = original_marker.bytes().take_while(|&b| b == b'#').count();
        let heading = (1..=6).contains(&hashes)
            && original_marker
                .as_bytes()
                .get(hashes)
                .is_none_or(u8::is_ascii_whitespace);
        if heading {
            let mut title = line[hashes..].trim();
            let trimmed = title.trim_end_matches('#');
            if trimmed.len() < title.len() && (trimmed.is_empty() || trimmed.ends_with([' ', '\t']))
            {
                title = trimmed.trim_end();
            }
            if hashes == 2 {
                value.h2_count += 1;
                value.empty_title |= title.is_empty();
            } else if hashes == 1 {
                value.other_top_level = true;
            } else if value.h2_count == 0 {
                value.before_heading = true;
            }
            previous_paragraph = false;
            continue;
        }
        // Reference definitions are invisible block content, not visible prose.
        // This small profile rejects them rather than parsing multiline definitions.
        if line.starts_with('[') && is_reference_definition(&lines[line_index..].join("\n")) {
            value.reference_definition = true;
            previous_paragraph = false;
            continue;
        }
        if previous_paragraph
            && (line.bytes().all(|b| b == b'=') || line.bytes().all(|b| b == b'-'))
        {
            value.setext = true;
        }
        if value.h2_count == 0 {
            value.before_heading = true;
        } else if has_prose(line) {
            value.prose = true;
        }
        previous_paragraph = true;
    }
    value.unclosed_block = comment || fence.is_some();
    value
}

fn is_reference_definition(text: &str) -> bool {
    let text = text.trim_start_matches(' ');
    if !text.starts_with('[') {
        return false;
    }
    let mut escaped = false;
    for (index, c) in text.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == '[' {
            return false;
        }
        if c == ']' {
            let label = &text[1..index];
            return !label.trim().is_empty()
                && !label.contains("\n\n")
                && text[index + 1..].starts_with(':');
        }
    }
    false
}
