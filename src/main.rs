use std::{
    env,
    error::Error,
    fmt, fs,
    io::{self, Read},
    process::ExitCode,
};

use assembler::{
    assemble::{encode::Encoder, page_checker::PageChecker},
    diagnostics::{FileId, Partial, print_diagnostics},
    frontend::{
        analysis::{symbol_table::SymbolTable, validation::Validator},
        syntax::{
            parser::Parser,
            tokens::{Token, TokenKind},
        },
    },
    preprocessing::{PreprocessedSource, Preprocessor},
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> CliResult<ExitCode> {
    let cli = match parse_args(env::args().skip(1))? {
        CliAction::Run(cli) => cli,
        CliAction::Exit(code) => return Ok(code),
    };

    let (display_name, source) = read_input(&cli.input)?;
    let diagnostics = if cli.preprocess_only {
        preprocess_source(&source, &cli.defines)
    } else {
        compile_source(&source, &cli.defines)
    };

    if !diagnostics.diagnostics.is_empty() {
        let mut stderr = io::stderr().lock();
        print_diagnostics(
            &mut stderr,
            &display_name,
            &source,
            &diagnostics.diagnostics,
        )
        .map_err(|error| CliError::with_source("failed to write diagnostics", error))?;
        return Ok(ExitCode::FAILURE);
    }

    match diagnostics.value {
        Some(Output::Binary(image)) => {
            if let Some(output_path) = cli.output {
                fs::write(&output_path, &image).map_err(|error| {
                    CliError::with_source(
                        format!("failed to write output file `{output_path}`"),
                        error,
                    )
                })?;
            }
        }
        Some(Output::Text(text)) => {
            print!("{text}");
        }
        None => {}
    }

    Ok(ExitCode::SUCCESS)
}

#[derive(Debug, PartialEq, Eq)]
struct Cli {
    input: String,
    output: Option<String>,
    preprocess_only: bool,
    defines: Vec<(String, String)>,
}

#[derive(Debug, PartialEq, Eq)]
enum Output {
    Binary(Vec<u8>),
    Text(String),
}

#[derive(Debug)]
enum CliAction {
    Run(Cli),
    Exit(ExitCode),
}

fn parse_args<I>(args: I) -> CliResult<CliAction>
where
    I: IntoIterator<Item = String>,
{
    let mut input = None;
    let mut output = None;
    let mut preprocess_only = false;
    let mut defines = Vec::new();
    let mut positional_only = false;

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if positional_only {
            if input.replace(arg).is_some() {
                return Err(CliError::new("expected exactly one input path"));
            }
            continue;
        }

        match arg.as_str() {
            "--" => positional_only = true,
            "-h" | "--help" => {
                print_usage();
                return Ok(CliAction::Exit(ExitCode::SUCCESS));
            }
            "-v" | "--version" => {
                println!("assembler {}", env!("CARGO_PKG_VERSION"));
                return Ok(CliAction::Exit(ExitCode::SUCCESS));
            }
            "-E" | "-e" | "--preprocess-only" => preprocess_only = true,
            "-o" => {
                let Some(path) = args.next() else {
                    return Err(CliError::new("missing path after `-o`"));
                };
                output = Some(path);
            }
            "-D" => {
                let Some(define) = args.next() else {
                    return Err(CliError::new("missing macro after `-D`"));
                };
                defines.push(parse_define_arg(&define)?);
            }
            _ if arg.starts_with("-D") => {
                let Some(define) = arg.strip_prefix("-D") else {
                    unreachable!();
                };
                if define.is_empty() {
                    return Err(CliError::new("missing macro after `-D`"));
                }
                defines.push(parse_define_arg(define)?);
            }
            _ if arg.starts_with('-') && arg != "-" => {
                return Err(CliError::new(format!("unknown option `{arg}`")));
            }
            _ => {
                if input.replace(arg).is_some() {
                    return Err(CliError::new("expected exactly one input path"));
                }
            }
        }
    }

    let Some(input) = input else {
        eprintln!("assembler: fatal: no input file specified");
        eprintln!("Type assembler -h for help.");
        return Ok(CliAction::Exit(ExitCode::FAILURE));
    };

    Ok(CliAction::Run(Cli {
        input,
        output,
        preprocess_only,
        defines,
    }))
}

fn parse_define_arg(arg: &str) -> CliResult<(String, String)> {
    let (name, replacement) = match arg.split_once('=') {
        Some((name, replacement)) => (name, replacement),
        None => (arg, "1"),
    };

    if name.is_empty() {
        return Err(CliError::new("expected macro name in `-D`"));
    }

    Ok((name.to_owned(), replacement.to_owned()))
}

fn read_input(input: &str) -> CliResult<(String, String)> {
    if input == "-" {
        let mut source = String::new();
        io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| CliError::with_source("failed to read source from stdin", error))?;
        Ok(("<stdin>".to_owned(), source))
    } else {
        let source = fs::read_to_string(input).map_err(|error| {
            CliError::with_source(format!("failed to read source file `{input}`"), error)
        })?;
        Ok((input.to_owned(), source))
    }
}

type CliResult<T> = Result<T, CliError>;

#[derive(Debug)]
struct CliError {
    message: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    fn with_source(message: impl Into<String>, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(source) = &self.source {
            write!(f, ": {source}")?;
        }
        Ok(())
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

fn preprocess_source(source: &str, defines: &[(String, String)]) -> Partial<Output> {
    Preprocessor::new()
        .preprocess_with_defines(0 as FileId, source, defines)
        .map(|preprocessed| Output::Text(render_preprocessed_source(&preprocessed)))
}

fn compile_source(source: &str, defines: &[(String, String)]) -> Partial<Output> {
    let mut diagnostics = Vec::new();

    let preprocessed = Preprocessor::new().preprocess_with_defines(0 as FileId, source, defines);
    diagnostics.extend(preprocessed.diagnostics);
    let Some(preprocessed) = preprocessed.value else {
        return Partial::failure(diagnostics);
    };

    let parsed = Parser::new(&preprocessed.tokens).parse();
    diagnostics.extend(parsed.diagnostics);
    let Some(program) = parsed.value else {
        return Partial::failure(diagnostics);
    };

    let validation = Validator::new().validate_program(&program);
    let validation_has_errors = !validation.diagnostics.is_empty();
    diagnostics.extend(validation.diagnostics);
    if validation_has_errors {
        return Partial::failure(diagnostics);
    }

    let symbols = SymbolTable::build(&program);
    let Some(symbols) = symbols.value else {
        return Partial::failure(diagnostics);
    };

    let page_check = PageChecker::new().analyze(&program, &symbols);
    let page_check_has_errors = !page_check.diagnostics.is_empty();
    diagnostics.extend(page_check.diagnostics);
    if page_check_has_errors {
        return Partial::failure(diagnostics);
    }

    let assembled = Encoder::new().assemble(&program);
    diagnostics.extend(assembled.diagnostics);

    if diagnostics.is_empty() {
        Partial::with_diagnostics(
            Output::Binary(assembled.value.unwrap_or_default()),
            diagnostics,
        )
    } else {
        Partial {
            value: assembled.value.map(Output::Binary),
            diagnostics,
        }
    }
}

fn print_usage() {
    eprintln!("Usage: assembler [options...] [--] <input.asm | ->");
    eprintln!("       assembler -v");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -h, --help             show this text and exit");
    eprintln!("  -v, --version          print the assembler version and exit");
    eprintln!("  -o <file>              write assembled output to <file>");
    eprintln!("  -E, -e                 preprocess only, writing to stdout");
    eprintln!("  -Dname[=value]         pre-define a preprocessor symbol");
    eprintln!("  --                     stop option parsing");
}

fn render_preprocessed_source(preprocessed: &PreprocessedSource) -> String {
    let mut rendered = String::new();
    let mut previous: Option<&TokenKind> = None;

    for token in &preprocessed.tokens {
        match &token.kind {
            TokenKind::Eof => break,
            TokenKind::Newline => {
                rendered.push('\n');
                previous = None;
            }
            current => {
                if needs_space(previous, current) {
                    rendered.push(' ');
                }
                rendered.push_str(&render_token(token));
                previous = Some(current);
            }
        }
    }

    rendered
}

fn needs_space(previous: Option<&TokenKind>, current: &TokenKind) -> bool {
    let Some(previous) = previous else {
        return false;
    };

    if matches!(previous, TokenKind::Comma) {
        return !matches!(current, TokenKind::Newline | TokenKind::Eof);
    }

    matches!(
        previous,
        TokenKind::Identifier(_)
            | TokenKind::Integer { .. }
            | TokenKind::String(_)
            | TokenKind::Char { .. }
            | TokenKind::RBracket
    ) && matches!(
        current,
        TokenKind::Identifier(_)
            | TokenKind::Integer { .. }
            | TokenKind::String(_)
            | TokenKind::Char { .. }
            | TokenKind::LBracket
    )
}

fn render_token(token: &Token) -> String {
    match &token.kind {
        TokenKind::Identifier(name) => name.clone(),
        TokenKind::Integer { raw, .. } => raw.clone(),
        TokenKind::String(value) => format!("{value:?}"),
        TokenKind::Char { raw, .. } => format!("'{raw}'"),
        TokenKind::Dot => ".".to_owned(),
        TokenKind::Comma => ",".to_owned(),
        TokenKind::Colon => ":".to_owned(),
        TokenKind::At => "@".to_owned(),
        TokenKind::Question => "?".to_owned(),
        TokenKind::Excl => "!".to_owned(),
        TokenKind::LBracket => "[".to_owned(),
        TokenKind::RBracket => "]".to_owned(),
        TokenKind::Plus => "+".to_owned(),
        TokenKind::Minus => "-".to_owned(),
        TokenKind::Newline | TokenKind::Eof => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, CliAction, Output, parse_args, parse_define_arg, preprocess_source};
    use std::process::ExitCode;

    #[test]
    fn parses_input_only() {
        let CliAction::Run(cli) = parse_args(["program.asm".to_owned()]).unwrap() else {
            panic!("expected run action");
        };
        assert_eq!(
            cli,
            Cli {
                input: "program.asm".to_owned(),
                output: None,
                preprocess_only: false,
                defines: Vec::new(),
            }
        );
    }

    #[test]
    fn parses_output_option_after_input() {
        let CliAction::Run(cli) = parse_args([
            "program.asm".to_owned(),
            "-o".to_owned(),
            "out.bin".to_owned(),
        ])
        .unwrap() else {
            panic!("expected run action");
        };
        assert_eq!(
            cli,
            Cli {
                input: "program.asm".to_owned(),
                output: Some("out.bin".to_owned()),
                preprocess_only: false,
                defines: Vec::new(),
            }
        );
    }

    #[test]
    fn parses_output_option_before_input() {
        let CliAction::Run(cli) = parse_args([
            "-o".to_owned(),
            "out.bin".to_owned(),
            "program.asm".to_owned(),
        ])
        .unwrap() else {
            panic!("expected run action");
        };
        assert_eq!(
            cli,
            Cli {
                input: "program.asm".to_owned(),
                output: Some("out.bin".to_owned()),
                preprocess_only: false,
                defines: Vec::new(),
            }
        );
    }

    #[test]
    fn rejects_missing_output_path() {
        let error = parse_args(["program.asm".to_owned(), "-o".to_owned()]).unwrap_err();
        assert_eq!(error.to_string(), "missing path after `-o`");
    }

    #[test]
    fn parses_preprocess_only_and_define_flags() {
        let CliAction::Run(cli) = parse_args([
            "-E".to_owned(),
            "-DFOO=42".to_owned(),
            "-D".to_owned(),
            "BAR".to_owned(),
            "program.asm".to_owned(),
        ])
        .unwrap() else {
            panic!("expected run action");
        };

        assert_eq!(
            cli,
            Cli {
                input: "program.asm".to_owned(),
                output: None,
                preprocess_only: true,
                defines: vec![
                    ("FOO".to_owned(), "42".to_owned()),
                    ("BAR".to_owned(), "1".to_owned()),
                ],
            }
        );
    }

    #[test]
    fn parses_input_after_double_dash() {
        let CliAction::Run(cli) = parse_args(["--".to_owned(), "-program.asm".to_owned()]).unwrap()
        else {
            panic!("expected run action");
        };

        assert_eq!(
            cli,
            Cli {
                input: "-program.asm".to_owned(),
                output: None,
                preprocess_only: false,
                defines: Vec::new(),
            }
        );
    }

    #[test]
    fn parse_define_defaults_to_one() {
        assert_eq!(
            parse_define_arg("FLAG").unwrap(),
            ("FLAG".to_owned(), "1".to_owned())
        );
    }

    #[test]
    fn preprocess_only_expands_cli_defines() {
        let output = preprocess_source("lim r0, VALUE\n", &[("VALUE".to_owned(), "42".to_owned())])
            .into_result()
            .unwrap();

        assert_eq!(output, Output::Text("lim r0, 42\n".to_owned()));
    }

    #[test]
    fn missing_input_exits_failure() {
        let action = parse_args(Vec::<String>::new()).unwrap();
        assert!(matches!(action, CliAction::Exit(ExitCode::FAILURE)));
    }
}
