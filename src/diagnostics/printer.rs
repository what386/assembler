use std::io::{self, Write};

use crate::diagnostics::{Diagnostic, DiagnosticLabel, DiagnosticLabelKind};

pub fn print_diagnostics(
    writer: &mut impl Write,
    path: &str,
    source: &str,
    diagnostics: &[Diagnostic],
) -> io::Result<()> {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        print_diagnostic(writer, path, source, diagnostic)?;
        if index + 1 != diagnostics.len() {
            writeln!(writer)?;
        }
    }

    Ok(())
}

fn print_diagnostic(
    writer: &mut impl Write,
    path: &str,
    source: &str,
    diagnostic: &Diagnostic,
) -> io::Result<()> {
    if let Some(label) = primary_label(diagnostic).or_else(|| diagnostic.labels.first()) {
        let line_info = line_info(source, label.span.start);
        if let Some(code) = &diagnostic.code {
            writeln!(
                writer,
                "{}[{}]: {}",
                diagnostic.severity.as_str(),
                code.as_str(),
                diagnostic.message
            )?;
        } else {
            writeln!(
                writer,
                "{}: {}",
                diagnostic.severity.as_str(),
                diagnostic.message,
            )?;
        }
        writeln!(
            writer,
            "  --> {path} [{}:{}]",
            line_info.number, line_info.column
        )?;
    } else if let Some(code) = &diagnostic.code {
        writeln!(
            writer,
            "{}[{}]: {}",
            diagnostic.severity.as_str(),
            code.as_str(),
            diagnostic.message
        )?;
    } else {
        writeln!(
            writer,
            "{}: {}",
            diagnostic.severity.as_str(),
            diagnostic.message
        )?;
    }

    let mut labels = diagnostic.labels.iter().collect::<Vec<_>>();
    labels.sort_by_key(|label| (label.span.start, label.span.end));
    let gutter_width = labels
        .iter()
        .map(|label| line_info(source, label.span.start).number)
        .max()
        .unwrap_or(0)
        .to_string()
        .len()
        .max(1);

    let mut previous_line = None;
    for label in labels {
        let current_line = line_info(source, label.span.start).number;
        if let Some(previous_line) = previous_line
            && current_line > previous_line + 1
        {
            print_ellipsis(writer, gutter_width)?;
        }
        print_label(writer, source, diagnostic, label, gutter_width)?;
        previous_line = Some(current_line);
    }

    Ok(())
}

fn print_label(
    writer: &mut impl Write,
    source: &str,
    diagnostic: &Diagnostic,
    label: &DiagnosticLabel,
    gutter_width: usize,
) -> io::Result<()> {
    let line_info = line_info(source, label.span.start);
    let marker = match label.kind {
        DiagnosticLabelKind::Primary => '^',
        DiagnosticLabelKind::Secondary => '-',
    };
    let underline_start = line_info.column.saturating_sub(1);
    let underline_len = underline_length(label, &line_info);
    let detail = if label.message.is_empty()
        || (matches!(label.kind, DiagnosticLabelKind::Primary)
            && label.message == diagnostic.message)
    {
        String::new()
    } else {
        format!(" {}", label.message)
    };
    writeln!(
        writer,
        "{:>gutter_width$} | {}",
        line_info.number, line_info.text
    )?;
    if matches!(label.kind, DiagnosticLabelKind::Secondary) && detail.is_empty() {
        return Ok(());
    }
    writeln!(
        writer,
        "{:>gutter_width$} | {}{}{}",
        "",
        " ".repeat(underline_start),
        marker.to_string().repeat(underline_len),
        detail,
    )?;

    Ok(())
}

fn print_ellipsis(writer: &mut impl Write, gutter_width: usize) -> io::Result<()> {
    let _ = gutter_width;
    writeln!(writer, "...")?;
    Ok(())
}

fn primary_label(diagnostic: &Diagnostic) -> Option<&DiagnosticLabel> {
    diagnostic
        .labels
        .iter()
        .find(|label| matches!(label.kind, DiagnosticLabelKind::Primary))
}

fn underline_length(label: &DiagnosticLabel, line_info: &LineInfo<'_>) -> usize {
    if label.span.is_empty() {
        return 1;
    }

    let line_end = line_info.end;
    let clamped_end = label.span.end.min(line_end).max(label.span.start + 1);
    clamped_end.saturating_sub(label.span.start).max(1)
}

struct LineInfo<'a> {
    number: usize,
    column: usize,
    text: &'a str,
    end: usize,
}

fn line_info(source: &str, offset: usize) -> LineInfo<'_> {
    let clamped = offset.min(source.len());
    let bytes = source.as_bytes();
    let mut line = 1usize;
    let mut line_start = 0usize;
    let mut index = 0usize;

    while index < clamped {
        match bytes[index] {
            b'\r' => {
                line += 1;
                if bytes.get(index + 1) == Some(&b'\n') {
                    index += 1;
                }
                index += 1;
                line_start = index;
            }
            b'\n' => {
                line += 1;
                index += 1;
                line_start = index;
            }
            _ => index += 1,
        }
    }

    let mut line_end = line_start;
    while line_end < bytes.len() {
        match bytes[line_end] {
            b'\r' | b'\n' => break,
            _ => line_end += 1,
        }
    }

    LineInfo {
        number: line,
        column: clamped.saturating_sub(line_start) + 1,
        text: &source[line_start..line_end],
        end: line_end,
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostics::{
        Diagnostic, DiagnosticCode, DiagnosticLabel, DiagnosticLabelKind, Span, print_diagnostics,
    };

    fn render(source: &str, diagnostics: &[Diagnostic]) -> String {
        let mut out = Vec::new();
        print_diagnostics(&mut out, "sample.asm", source, diagnostics).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn suppresses_duplicate_primary_label_message() {
        let diagnostic = Diagnostic::error_code(DiagnosticCode::EncodingError(
            "branch target crosses a 64-instruction page boundary".to_owned(),
        ))
        .with_label(DiagnosticLabel::new(
            Span::new(0, 4, 7),
            "branch target crosses a 64-instruction page boundary",
        ));

        let rendered = render("bra done, ?equal\n", &[diagnostic]);

        assert!(rendered.contains("error[E011]"));
        assert!(rendered.contains("  --> sample.asm [1:5]"));
        assert!(rendered.contains("branch target crosses a 64-instruction page boundary"));
        assert!(rendered.contains("1 | bra done, ?equal"));
        assert!(rendered.contains("|     ^^^"));
        assert!(!rendered.contains("\n  -> sample.asm:1:5"));
    }

    #[test]
    fn renders_secondary_labels_with_context() {
        let diagnostic = Diagnostic::error_code(DiagnosticCode::InvalidDirective(
            "duplicate label `start`".to_owned(),
        ))
        .with_label(DiagnosticLabel::new(Span::new(0, 7, 12), "redefined here"))
        .with_label(DiagnosticLabel::secondary(
            Span::new(0, 0, 5),
            "previous definition here",
        ));

        let rendered = render("start:\nstart:\n", &[diagnostic]);

        assert!(rendered.contains("redefined here"));
        assert!(rendered.contains("----- previous definition here"));
    }

    #[test]
    fn suppresses_blank_secondary_label_underlines() {
        let diagnostic = Diagnostic::error_code(DiagnosticCode::EncodingError(
            "cross-page branch".to_owned(),
        ))
        .with_label(DiagnosticLabel::new(
            Span::new(0, 0, 3),
            "cross-page branch",
        ))
        .with_label(DiagnosticLabel::secondary(Span::new(0, 4, 9), ""));

        let rendered = render("bra\n.page 2\n", &[diagnostic]);

        assert!(rendered.contains("2 | .page 2"));
        assert!(!rendered.contains("2 | -----"));
    }

    #[test]
    fn renders_ellipsis_for_non_adjacent_labels() {
        let diagnostic = Diagnostic::error_code(DiagnosticCode::EncodingError(
            "cross-page branch".to_owned(),
        ))
        .with_label(DiagnosticLabel::new(
            Span::new(0, 0, 3),
            "cross-page branch",
        ))
        .with_label(DiagnosticLabel::secondary(Span::new(0, 6, 13), "later"));

        let rendered = render("bra\n\n\n.page 2\n", &[diagnostic]);

        assert!(rendered.contains("1 | bra"));
        assert!(rendered.contains("\n...\n"));
        assert!(rendered.contains(".page 2"));
    }

    #[test]
    fn renders_empty_spans_with_single_marker() {
        let diagnostic = Diagnostic::error_code(DiagnosticCode::InvalidDirective(
            "directive `.endif` expected a matching conditional".to_owned(),
        ))
        .with_label(DiagnosticLabel::new(
            Span::empty(0, 6),
            "directive `.endif` expected a matching conditional",
        ));

        let rendered = render("halt\n\n", &[diagnostic]);

        assert!(rendered.contains("| ^"));
    }

    #[test]
    fn handles_crlf_line_boundaries() {
        let diagnostic = Diagnostic::error_code(DiagnosticCode::UnexpectedToken(
            "unexpected token".to_owned(),
        ))
        .with_label(DiagnosticLabel {
            span: Span::new(0, 7, 10),
            message: "unexpected token".to_owned(),
            kind: DiagnosticLabelKind::Primary,
        });

        let rendered = render("halt\r\nfoo bar\r\n", &[diagnostic]);

        assert!(rendered.contains("--> sample.asm [2:2]"));
        assert!(rendered.contains("2 | foo bar"));
    }
}
