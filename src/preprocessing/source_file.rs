use crate::diagnostics::{Diagnostic, Span};

pub fn comment_start(line: &str) -> Option<usize> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;

    for (idx, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        match ch {
            '\\' if in_string || in_char => escaped = true,
            '"' if !in_char => in_string = !in_string,
            '\'' if !in_string => in_char = !in_char,
            ';' if !in_string && !in_char => return Some(idx),
            _ => {}
        }
    }

    None
}

pub fn mask_text(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\n' | '\r') {
            masked.push(ch);
        } else {
            for _ in 0..ch.len_utf8() {
                masked.push(' ');
            }
        }
    }
    masked
}

pub fn shift_diagnostic(mut diagnostic: Diagnostic, offset: usize) -> Diagnostic {
    for label in &mut diagnostic.labels {
        label.span = Span::new(
            label.span.file_id,
            label.span.start + offset,
            label.span.end + offset,
        );
    }
    diagnostic
}

pub fn iterate_lines(source: &str, mut f: impl FnMut(usize, &str, &str)) {
    let bytes = source.as_bytes();
    let mut start = 0;
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                if bytes.get(index + 1) == Some(&b'\n') {
                    let line = &source[start..index];
                    f(start, line, "\r\n");
                    index += 2;
                    start = index;
                    continue;
                } else {
                    let line = &source[start..index];
                    f(start, line, "\r");
                    index += 1;
                    start = index;
                    continue;
                };
            }
            b'\n' => {
                let line = &source[start..index];
                f(start, line, "\n");
                index += 1;
                start = index;
                continue;
            }
            _ => index += 1,
        }
    }

    if start < source.len() || source.is_empty() {
        f(start, &source[start..], "");
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        frontend::syntax::{
            parser::Parser,
            statements::{DirectiveArg, Statement},
            tokens::TokenKind,
        },
        preprocessing::Preprocessor,
    };

    #[test]
    fn strips_comments_without_moving_active_spans() {
        let source = "lim r0, 1 ; comment\nhalt\n";
        let processed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();

        assert_eq!(processed.masked_source.len(), source.len());
        assert_eq!(processed.tokens[0].span.start, 0);
        let halt_token = processed
            .tokens
            .iter()
            .find(|token| matches!(token.kind, TokenKind::Identifier(ref name) if name == "halt"))
            .unwrap();
        assert_eq!(halt_token.span.start, source.find("halt").unwrap());
    }

    #[test]
    fn ignores_comment_markers_inside_strings() {
        let source = ".string \";not comment\"\n";
        let processed = Preprocessor::new()
            .preprocess(0, source)
            .into_result()
            .unwrap();

        let program = Parser::new(&processed.tokens)
            .parse()
            .into_result()
            .unwrap();
        assert!(matches!(
            &program.statements[0],
            Statement::Directive(directive)
                if directive.args == vec![DirectiveArg::String(";not comment".to_owned())]
        ));
    }
}
