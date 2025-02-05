use anyhow::{Context, Result};
use clap::{Arg, Command};
use hgrep::grep::BufReadExt;
use hgrep::printer::{PrinterOptions, TextWrapMode};
use std::cmp;
use std::env;
use std::io;
use std::process;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "ripgrep")]
use hgrep::ripgrep;

#[cfg(feature = "bat-printer")]
use hgrep::bat::BatPrinter;

#[cfg(feature = "syntect-printer")]
use hgrep::syntect::SyntectPrinter;

fn command() -> Command {
    #[cfg(feature = "syntect-printer")]
    const DEFAULT_PRINTER: &str = "syntect";

    #[cfg(all(not(feature = "syntect-printer"), feature = "bat-printer"))]
    const DEFAULT_PRINTER: &str = "bat";

    let cmd = Command::new("hgrep")
        .version(env!("CARGO_PKG_VERSION"))
        .about(
            "hgrep is grep with human-friendly search output. It eats an output of `grep -nH` and prints the matches \
            with syntax-highlighted code snippets.\n\n\
            $ grep -nH pattern -R . | hgrep\n\n\
            For more details, visit https://github.com/rhysd/hgrep"
        )
        .arg(
            Arg::new("min-context")
                .short('c')
                .long("min-context")
                .num_args(1)
                .value_name("NUM")
                .default_value("3")
                .help("Minimum lines of leading and trailing context surrounding each match"),
        )
        .arg(
            Arg::new("max-context")
                .short('C')
                .long("max-context")
                .num_args(1)
                .value_name("NUM")
                .default_value("6")
                .help("Maximum lines of leading and trailing context surrounding each match"),
        )
        .arg(
            Arg::new("no-grid")
                .short('G')
                .long("no-grid")
                .help("Remove borderlines for more compact output"),
        )
        .arg(
            Arg::new("grid")
                .long("grid")
                .help("Add borderlines to output. This flag is an opposite of --no-grid"),
        )
        .arg(
            Arg::new("tab")
                .long("tab")
                .num_args(1)
                .value_name("NUM")
                .default_value("4")
                .help("Number of spaces for tab character. Set 0 to pass tabs through directly"),
        )
        .arg(
            Arg::new("theme")
                .long("theme")
                .num_args(1)
                .value_name("THEME")
                .help("Theme for syntax highlighting. Use --list-themes flag to print the theme list"),
        )
        .arg(
            Arg::new("list-themes")
                .long("list-themes")
                .help("List all available theme names and their samples. Samples show the output where 'let' is searched. The names can be used at --theme option"),
        )
        .arg(
            Arg::new("printer")
                .short('p')
                .long("printer")
                .value_name("PRINTER")
                .default_value(DEFAULT_PRINTER)
                .help("Printer to print the match results. 'bat' or 'syntect' is available"),
        )
        .arg(
            Arg::new("term-width")
                .long("term-width")
                .num_args(1)
                .value_name("NUM")
                .help("Width (number of characters) of terminal window"),
        ).arg(
            Arg::new("wrap")
                .long("wrap")
                .num_args(1)
                .value_name("MODE")
                .default_value("char")
                .value_parser(["char", "never"])
                .ignore_case(true)
                .help("Text-wrapping mode. 'char' enables character-wise text-wrapping. 'never' disables text-wrapping")
        ).arg(
            Arg::new("first-only")
                .short('f')
                .long("first-only")
                .help("Show only the first code snippet per file")
        )
        .arg(
            Arg::new("generate-completion-script")
                .long("generate-completion-script")
                .num_args(1)
                .value_name("SHELL")
                .value_parser(["bash", "zsh", "powershell", "fish", "elvish"])
                .ignore_case(true)
                .help("Print completion script for SHELL to stdout"),
        );

    #[cfg(feature = "bat-printer")]
    let cmd = cmd.arg(
        Arg::new("custom-assets")
            .long("custom-assets")
            .help("Load bat's custom assets. Note that this flag may not work with some version of `bat` command. This flag is only for bat printer"),
    );

    #[cfg(feature = "syntect-printer")]
    let cmd = cmd
        .arg(
            Arg::new("background")
                .long("background")
                .help("Paint background colors. This flag is only for syntect printer"),
        )
        .arg(
            Arg::new("ascii-lines").long("ascii-lines").help(
                "Use ASCII characters for drawing border lines instead of Unicode characters",
            ),
        );

    #[cfg(feature = "ripgrep")]
    let cmd = cmd
            .about(
                "hgrep is grep with human-friendly search output. It eats an output of `grep -nH` and prints the \
                matches with syntax-highlighted code snippets.\n\n\
                $ grep -nH pattern -R . | hgrep\n\n\
                hgrep has its builtin grep implementation. It's subset of ripgrep and faster when many matches are found.\n\n\
                $ hgrep pattern\n\n\
                For more details, visit https://github.com/rhysd/hgrep"
            )
            .override_usage("hgrep [FLAGS] [OPTIONS] [PATTERN [PATH...]]")
            .arg(
                Arg::new("no-ignore")
                    .long("no-ignore")
                    .help("Don't respect ignore files (.gitignore, .ignore, etc.)"),
            )
            .arg(
                Arg::new("ignore-case")
                    .short('i')
                    .long("ignore-case")
                    .help("When this flag is provided, the given pattern will be searched case insensitively"),
            )
            .arg(
                Arg::new("smart-case")
                    .short('S')
                    .long("smart-case")
                    .help("Search case insensitively if the pattern is all lowercase. Search case sensitively otherwise"),
            )
            .arg(
                Arg::new("hidden")
                    .short('.')
                    .long("hidden")
                    .help("Search hidden files and directories. By default, hidden files and directories are skipped"),
            )
            .arg(
                Arg::new("glob")
                    .short('g')
                    .long("glob")
                    .num_args(1)
                    .value_name("GLOB")
                    .num_args(1..)
                    .allow_hyphen_values(true)
                    .help("Include or exclude files and directories for searching that match the given glob"),
            )
            .arg(
                Arg::new("glob-case-insensitive")
                    .long("glob-case-insensitive")
                    .help("Process glob patterns given with the -g/--glob flag case insensitively"),
            )
            .arg(
                Arg::new("fixed-strings")
                    .short('F')
                    .long("fixed-strings")
                    .help("Treat the pattern as a literal string instead of a regular expression"),
            )
            .arg(
                Arg::new("word-regexp")
                    .short('w')
                    .long("word-regexp")
                    .help("Only show matches surrounded by word boundaries"),
            )
            .arg(
                Arg::new("follow-symlink")
                    .short('L')
                    .long("follow")
                    .help("When this flag is enabled, hgrep will follow symbolic links while traversing directories"),
            )
            .arg(
                Arg::new("multiline")
                    .short('U')
                    .long("multiline")
                    .help("Enable matching across multiple lines"),
            )
            .arg(
                Arg::new("multiline-dotall")
                    .long("multiline-dotall")
                    .help("Enable \"dot all\" in your regex pattern, which causes '.' to match newlines when multiline searching is enabled"),
            )
            .arg(
                Arg::new("crlf")
                    .long("crlf")
                    .help(r"When enabled, hgrep will treat CRLF ('\r\n') as a line terminator instead of just '\n'. This flag is useful on Windows"),
            )
            .arg(
                Arg::new("mmap")
                    .long("mmap")
                    .help("Search using memory maps when possible. mmap is disabled by default unlike ripgrep"),
            )
            .arg(
                Arg::new("max-count")
                    .short('m')
                    .long("max-count")
                    .num_args(1)
                    .value_name("NUM")
                    .help("Limit the number of matching lines per file searched to NUM"),
            )
            .arg(
                Arg::new("max-depth")
                    .long("max-depth")
                    .num_args(1)
                    .value_name("NUM")
                    .help("Limit the depth of directory traversal to NUM levels beyond the paths given"),
            )
            .arg(
                Arg::new("line-regexp")
                    .short('x')
                    .long("line-regexp")
                    .help("Only show matches surrounded by line boundaries. This is equivalent to putting ^...$ around the search pattern"),
            )
            .arg(
                Arg::new("pcre2")
                    .short('P')
                    .long("pcre2")
                    .help("When this flag is present, hgrep will use the PCRE2 regex engine instead of its default regex engine"),
            )
            .arg(
                Arg::new("type")
                    .short('t')
                    .long("type")
                    .num_args(1)
                    .value_name("TYPE")
                    .action(clap::ArgAction::Append)
                    .help("Only search files matching TYPE. This option is repeatable. --type-list can print the list of types"),
            )
            .arg(
                Arg::new("type-not")
                    .short('T')
                    .long("type-not")
                    .num_args(1)
                    .value_name("TYPE")
                    .action(clap::ArgAction::Append)
                    .help("Do not search files matching TYPE. Inverse of --type. This option is repeatable. --type-list can print the list of types"),
            )
            .arg(
                Arg::new("type-list")
                    .long("type-list")
                    .help("Show all supported file types and their corresponding globs"),
            )
            .arg(
                Arg::new("max-filesize")
                    .long("max-filesize")
                    .num_args(1)
                    .value_name("NUM+SUFFIX?")
                    .help("Ignore files larger than NUM in size. This does not apply to directories.The input format accepts suffixes of K, M or G which correspond to kilobytes, megabytes and gigabytes, respectively. If no suffix is provided the input is treated as bytes"),
            )
            .arg(
                Arg::new("invert-match")
                    .short('v')
                    .long("invert-match")
                    .help("Invert matching. Show lines that do not match the given pattern"),
            )
            .arg(
                Arg::new("one-file-system")
                    .long("one-file-system")
                    .help("When enabled, the search will not cross file system boundaries relative to where it started from"),
            )
            .arg(
                Arg::new("no-unicode")
                    .long("no-unicode")
                    .help("Disable unicode-aware regular expression matching"),
            )
            .arg(
                Arg::new("regex-size-limit")
                    .long("regex-size-limit")
                    .num_args(1)
                    .value_name("NUM+SUFFIX?")
                    .help("The upper size limit of the compiled regex. The default limit is 10M. For the size suffixes, see --max-filesize"),
            )
            .arg(
                Arg::new("dfa-size-limit")
                    .long("dfa-size-limit")
                    .num_args(1)
                    .value_name("NUM+SUFFIX?")
                    .help("The upper size limit of the regex DFA. The default limit is 10M. For the size suffixes, see --max-filesize"),
            )
            .arg(
                Arg::new("PATTERN")
                    .help("Pattern to search. Regular expression is available"),
            )
            .arg(
                Arg::new("PATH")
                    .help("Paths to search")
                    .num_args(0..)
                    .value_hint(clap::ValueHint::AnyPath)
                    .value_parser(clap::builder::ValueParser::path_buf()),
            );

    cmd
}

fn generate_completion_script(shell: &str) {
    use clap_complete::generate;
    use clap_complete::shells::*;

    let mut cmd = command();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if shell.eq_ignore_ascii_case("bash") {
        generate(Bash, &mut cmd, "hgrep", &mut stdout)
    } else if shell.eq_ignore_ascii_case("zsh") {
        generate(Zsh, &mut cmd, "hgrep", &mut stdout)
    } else if shell.eq_ignore_ascii_case("powershell") {
        generate(PowerShell, &mut cmd, "hgrep", &mut stdout)
    } else if shell.eq_ignore_ascii_case("fish") {
        generate(Fish, &mut cmd, "hgrep", &mut stdout)
    } else if shell.eq_ignore_ascii_case("elvish") {
        generate(Elvish, &mut cmd, "hgrep", &mut stdout)
    } else {
        unreachable!() // SHELL argument was validated by clap
    }
}

#[cfg(feature = "ripgrep")]
fn build_ripgrep_config(
    min_context: u64,
    max_context: u64,
    matches: &clap::ArgMatches,
) -> Result<ripgrep::Config<'_>> {
    let mut config = ripgrep::Config::default();
    config
        .min_context(min_context)
        .max_context(max_context)
        .no_ignore(matches.contains_id("no-ignore"))
        .hidden(matches.contains_id("hidden"))
        .case_insensitive(matches.contains_id("ignore-case"))
        .smart_case(matches.contains_id("smart-case"))
        .glob_case_insensitive(matches.contains_id("glob-case-insensitive"))
        .pcre2(matches.contains_id("pcre2")) // must be before fixed_string
        .fixed_strings(matches.contains_id("fixed-strings"))
        .word_regexp(matches.contains_id("word-regexp"))
        .follow_symlink(matches.contains_id("follow-symlink"))
        .multiline(matches.contains_id("multiline"))
        .crlf(matches.contains_id("crlf"))
        .multiline_dotall(matches.contains_id("multiline-dotall"))
        .mmap(matches.contains_id("mmap"))
        .line_regexp(matches.contains_id("line-regexp"))
        .invert_match(matches.contains_id("invert-match"))
        .one_file_system(matches.contains_id("one-file-system"))
        .no_unicode(matches.contains_id("no-unicode"));

    if let Some(globs) = matches.get_many::<String>("glob") {
        config.globs(globs.map(String::as_str));
    }

    if let Some(num) = matches.get_one::<String>("max-count") {
        let num = num
            .parse()
            .context("could not parse --max-count option value as unsigned integer")?;
        config.max_count(num);
    }

    if let Some(num) = matches.get_one::<String>("max-depth") {
        let num = num
            .parse()
            .context("could not parse --max-depth option value as unsigned integer")?;
        config.max_depth(num);
    }

    if let Some(size) = matches.get_one::<String>("max-filesize") {
        config
            .max_filesize(size)
            .context("coult not parse --max-filesize option value as file size string")?;
    }

    if let Some(limit) = matches.get_one::<String>("regex-size-limit") {
        config
            .regex_size_limit(limit)
            .context("coult not parse --regex-size-limit option value as size string")?;
    }

    if let Some(limit) = matches.get_one::<String>("dfa-size-limit") {
        config
            .dfa_size_limit(limit)
            .context("coult not parse --dfa-size-limit option value as size string")?;
    }

    let types = matches.get_many::<String>("type");
    if let Some(types) = types {
        config.types(types.map(String::as_str));
    }

    let types_not = matches.get_many::<String>("type-not");
    if let Some(types_not) = types_not {
        config.types_not(types_not.map(String::as_str));
    }

    Ok(config)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrinterKind {
    #[cfg(feature = "bat-printer")]
    Bat,
    #[cfg(feature = "syntect-printer")]
    Syntect,
}

fn app() -> Result<bool> {
    let matches = command().get_matches();
    if let Some(shell) = matches.get_one::<String>("generate-completion-script") {
        generate_completion_script(shell);
        return Ok(true);
    }

    #[allow(unused_variables)] // printer_kind is unused when syntect-printer is disabled for now
    let printer_kind = match matches.get_one::<String>("printer").unwrap().as_str() {
        #[cfg(feature = "bat-printer")]
        "bat" => PrinterKind::Bat,
        #[cfg(not(feature = "bat-printer"))]
        "bat" => anyhow::bail!("--printer bat is not available because 'bat-printer' feature was disabled at compilation"),
        #[cfg(feature = "syntect-printer")]
        "syntect" => PrinterKind::Syntect,
        #[cfg(not(feature = "syntect-printer"))]
        "syntect" => anyhow::bail!("--printer syntect is not available because 'syntect-printer' feature was disabled at compilation"),
        p => anyhow::bail!("Unknown printer '{}', at --printer option. It must be one of 'bat' or 'syntect'", p),
    };

    let min_context = matches
        .get_one::<String>("min-context")
        .unwrap()
        .parse()
        .context("could not parse \"min-context\" option value as unsigned integer")?;
    let max_context = matches
        .get_one::<String>("max-context")
        .unwrap()
        .parse()
        .context("could not parse \"max-context\" option value as unsigned integer")?;
    let max_context = cmp::max(min_context, max_context);

    let mut printer_opts = PrinterOptions::default();
    if let Some(width) = matches.get_one::<String>("tab") {
        printer_opts.tab_width = width
            .parse()
            .context("could not parse \"tab\" option value as unsigned integer")?;
    }

    #[cfg(feature = "bat-printer")]
    let theme_env = env::var("BAT_THEME").ok();
    #[cfg(feature = "bat-printer")]
    if printer_kind == PrinterKind::Bat {
        if let Some(var) = &theme_env {
            printer_opts.theme = Some(var);
        }
    }
    if let Some(theme) = matches.get_one::<String>("theme") {
        printer_opts.theme = Some(theme);
    }

    let is_grid = matches.contains_id("grid");
    #[cfg(feature = "bat-printer")]
    if printer_kind == PrinterKind::Bat {
        if let Ok("plain" | "header" | "numbers") =
            env::var("BAT_STYLE").as_ref().map(String::as_str)
        {
            if !is_grid {
                printer_opts.grid = false;
            }
        }
    }
    if matches.contains_id("no-grid") && !is_grid {
        printer_opts.grid = false;
    }

    if let Some(width) = matches.get_one::<String>("term-width") {
        let width = width
            .parse()
            .context("could not parse \"term-width\" option value as unsigned integer")?;
        printer_opts.term_width = width;
        if width < 10 {
            anyhow::bail!("Too small value at --term-width option ({} < 10)", width);
        }
    }

    if let Some(mode) = matches.get_one::<String>("wrap") {
        if mode.eq_ignore_ascii_case("never") {
            printer_opts.text_wrap = TextWrapMode::Never;
        } else if mode.eq_ignore_ascii_case("char") {
            printer_opts.text_wrap = TextWrapMode::Char;
        } else {
            unreachable!(); // Option value was validated by clap
        }
    }

    if matches.contains_id("first-only") {
        printer_opts.first_only = true;
    }

    #[cfg(feature = "syntect-printer")]
    {
        if matches.contains_id("background") {
            printer_opts.background_color = true;
            #[cfg(feature = "bat-printer")]
            if printer_kind == PrinterKind::Bat {
                anyhow::bail!("--background flag is only available for syntect printer since bat does not support painting background colors");
            }
        }

        if matches.contains_id("ascii-lines") {
            printer_opts.ascii_lines = true;
            #[cfg(feature = "bat-printer")]
            if printer_kind == PrinterKind::Bat {
                anyhow::bail!("--ascii-lines flag is only available for syntect printer since bat does not support this feature");
            }
        }
    }

    #[cfg(feature = "bat-printer")]
    if matches.contains_id("custom-assets") {
        printer_opts.custom_assets = true;
        #[cfg(feature = "syntect-printer")]
        if printer_kind == PrinterKind::Syntect {
            anyhow::bail!("--custom-assets flag is only available for bat printer");
        }
    }

    if matches.contains_id("list-themes") {
        #[cfg(feature = "syntect-printer")]
        if printer_kind == PrinterKind::Syntect {
            hgrep::syntect::list_themes(io::stdout().lock(), &printer_opts)?;
            return Ok(true);
        }

        #[cfg(feature = "bat-printer")]
        if printer_kind == PrinterKind::Bat {
            BatPrinter::new(printer_opts).list_themes()?;
            return Ok(true);
        }

        unreachable!();
    }

    #[cfg(feature = "ripgrep")]
    if matches.contains_id("type-list") {
        let config = build_ripgrep_config(min_context, max_context, &matches)?;
        config.print_types(io::stdout().lock())?;
        return Ok(true);
    }

    #[cfg(feature = "ripgrep")]
    if let Some(pattern) = matches.get_one::<String>("PATTERN") {
        use std::path::PathBuf;

        let paths = matches
            .get_many::<PathBuf>("PATH")
            .map(|p| p.map(PathBuf::as_path));
        let config = build_ripgrep_config(min_context, max_context, &matches)?;

        #[cfg(feature = "syntect-printer")]
        if printer_kind == PrinterKind::Syntect {
            let printer = SyntectPrinter::with_stdout(printer_opts)?;
            return ripgrep::grep(printer, pattern, paths, config);
        }

        #[cfg(feature = "bat-printer")]
        if printer_kind == PrinterKind::Bat {
            let printer = std::sync::Mutex::new(BatPrinter::new(printer_opts));
            return ripgrep::grep(printer, pattern, paths, config);
        }

        unreachable!();
    }

    #[cfg(feature = "syntect-printer")]
    if printer_kind == PrinterKind::Syntect {
        use hgrep::printer::Printer;
        use rayon::prelude::*;
        let printer = SyntectPrinter::with_stdout(printer_opts)?;
        return io::BufReader::new(io::stdin())
            .grep_lines()
            .chunks_per_file(min_context, max_context)
            .par_bridge()
            .map(|file| {
                printer.print(file?)?;
                Ok(true)
            })
            .try_reduce(|| false, |a, b| Ok(a || b));
    }

    #[cfg(feature = "bat-printer")]
    if printer_kind == PrinterKind::Bat {
        let mut found = false;
        let printer = BatPrinter::new(printer_opts);
        // XXX: io::stdin().lock() is not available since bat's implementation internally takes lock of stdin
        // *even if* it does not use stdin.
        // https://github.com/sharkdp/bat/issues/1902
        for f in io::BufReader::new(io::stdin())
            .grep_lines()
            .chunks_per_file(min_context, max_context)
        {
            printer.print(f?)?;
            found = true;
        }
        return Ok(found);
    }

    unreachable!();
}

fn main() {
    #[cfg(windows)]
    {
        ansi_term::enable_ansi_support().unwrap();
    }

    let status = match app() {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(err) => {
            eprintln!("\x1b[1;91merror:\x1b[0m {}", err);
            for err in err.chain().skip(1) {
                eprintln!("  Caused by: {}", err);
            }
            2
        }
    };
    process::exit(status);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parser() {
        command().debug_assert();
    }
}
