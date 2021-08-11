use argh::FromArgs;
use reqwest::blocking::{Client as BlockingClient, RequestBuilder};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::StatusCode;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

pub const PY27: &'static str = "27";
pub const PY33: &'static str = "33";
pub const PY34: &'static str = "34";
pub const PY35: &'static str = "35";
pub const PY36: &'static str = "36";
pub const PY37: &'static str = "37";
pub const PY38: &'static str = "38";
pub const PY39: &'static str = "39";
pub const PYI: &'static str = "PYI";

fn main() -> Result<(), String> {
    // Pull in and parse the arguments
    let cli_options: CliOptions = argh::from_env();

    if cli_options.src.is_empty() {
        println!("\nError: No target source file(s) specified!\n");
        return Ok(());
    }

    // Setup an instance of reqwest's blocking Client
    let client = BlockingClient::new();

    // Translate the launch arguments into their appropriate headers
    let headers = headers_from_cli_options(&cli_options);

    let req_builder = (&client)
        .post(format!(
            "http://{}:{}/",
            &(cli_options.host),
            &(cli_options.port)
        ))
        .headers(headers.clone());

    println!("\n");

    let (mut formatted, mut skipped) = (0u32, 0u32);

    for source_file in cli_options.src.iter() {
        match format_pyfile(
            source_file,
            req_builder.try_clone().unwrap_or(
                (&client)
                    .post(format!(
                        "http://{}:{}/",
                        &(cli_options.host),
                        &(cli_options.port)
                    ))
                    .headers(headers.clone()),
            ),
        ) {
            Ok(success) => {
                if success {
                    formatted += 1;
                } else {
                    skipped += 1;
                }
            }
            Err(err) => {
                skipped += 1;
                println!("{}", err);
            }
        }
    }

    let mut results: String = "\nAll done! ✨ 🍰 ✨".to_string();

    if formatted == 1 {
        results += "\n• 1 file reformatted";
    } else if formatted > 1 {
        results += format!("\n• {} files reformatted", formatted).as_str();
    }

    if skipped == 1 {
        results += "\n• 1 file left unchanged.";
    } else if skipped > 1 {
        results += format!("\n• {} files left unchanged.", skipped).as_str();
    }

    results += "\n";

    println!("{}", results);

    Ok(())
}

#[derive(Debug)]
struct BlackError {
    what_happened: String,
}

impl fmt::Display for BlackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.what_happened.as_str())
    }
}

impl BlackError {
    fn from_debug<T: fmt::Debug>(err: T) -> BlackError {
        BlackError {
            what_happened: format!("{:#?}", err),
        }
    }
}

impl From<std::io::Error> for BlackError {
    fn from(err: std::io::Error) -> Self {
        BlackError::from_debug(err)
    }
}

impl From<reqwest::Error> for BlackError {
    fn from(err: reqwest::Error) -> Self {
        BlackError::from_debug(err)
    }
}

impl From<std::string::FromUtf8Error> for BlackError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        BlackError::from_debug(err)
    }
}

impl Error for BlackError {}

#[derive(FromArgs, Debug, Default)]
/// black: The uncompromising code formatter
struct CliOptions {
    /// the address of the local blackd server [default: localhost]
    #[argh(option, short = 'h', default = "\"localhost\".to_string()")]
    host: String,

    /// the port the local blackd server is listening on [default: 45484]
    #[argh(option, short = 'p', default = "45484u16")]
    port: u16,

    /// how many characters per line to allow [default: 88]
    #[argh(option, short = 'l', default = "88")]
    line_length: u8,

    /// python versions that should be supported by Black's output [default: per-file auto-detection]
    #[argh(
        option,
        short = 't',
        default = "\"\".to_string()",
        from_str_fn(parse_py_versions)
    )]
    target_version: String,

    /// don't normalize string quotes or prefixes [default: false]
    #[argh(switch, short = 'S')]
    skip_string_normalization: bool,

    /// don't use trailing commas as a reason to split lines [default: false]
    #[argh(switch, short = 'C')]
    skip_magic_trailing_comma: bool,

    /// if --fast is given, skip temporary sanity checks [default: --safe]
    #[argh(switch)]
    fast: bool,

    /// if --safe is given, perform temporary sanity checks [default: --safe]
    #[argh(switch)]
    safe: bool,

    /// if present, the target source files will not be altered and a diff of the formats will be output instead [default: false]
    #[argh(switch)]
    diff: bool,

    /// the source file(s) to be formatted [required]
    #[argh(positional)]
    src: Vec<String>,
}

fn parse_py_versions(version_string: &str) -> Result<String, String> {
    let supported_versions = vec![PY27, PY33, PY34, PY35, PY36, PY37, PY38, PY39, PYI];

    let versions: Vec<String> = version_string
        .split(',')
        .filter(|entry| entry.len() > 0)
        .map(|item| item.chars().filter(|x| x.is_numeric()).collect())
        .collect::<Vec<String>>()
        .iter()
        .filter(|entry| supported_versions.contains(&&***entry))
        .map(|val| format!("py{}", val))
        .collect::<Vec<String>>();

    Ok(versions.join(",").to_string())
}

fn headers_from_cli_options(options: &CliOptions) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let line_length = (&options.line_length).to_string();

    // X-Protocol-Version
    headers.insert("X-Protocol-Version", HeaderValue::from_str("1").unwrap());

    // X-Line-Length
    headers.insert(
        "X-Line-Length",
        HeaderValue::from_str(line_length.as_str()).unwrap(),
    );

    // X-Skip-String-Normalization
    if options.skip_string_normalization {
        headers.insert(
            "X-Skip-String-Normalization",
            HeaderValue::from_str("true").unwrap(),
        );
    }

    // X-Skip-Magic-Trailing-Comma
    if options.skip_magic_trailing_comma {
        headers.insert(
            "X-Skip-Magic-Trailing-Comma",
            HeaderValue::from_str("true").unwrap(),
        );
    }

    // X-Fast-Or-Safe
    if options.fast && !options.safe {
        headers.insert("X-Fast-Or-Safe", HeaderValue::from_str("fast").unwrap());
    } else {
        headers.insert("X-Fast-Or-Safe", HeaderValue::from_str("safe").unwrap());
    }

    // X-Python-Variant
    if !options.target_version.is_empty() {
        headers.insert(
            "X-Python-Variant",
            HeaderValue::from_str(&options.target_version).unwrap(),
        );
    }

    // X-Diff
    if options.diff {
        headers.insert("X-Diff", HeaderValue::from_str("true").unwrap());
    }

    headers
}

fn read_pyfile(filepath: &Path) -> Result<Vec<u8>, BlackError> {
    // Grab a read-handle for the specified file
    let mut origin: fs::File = fs::OpenOptions::new().read(true).open(filepath)?;

    // Setup a mutable buffer for the file's contents
    let mut file_bytes: Vec<u8> = Vec::new();

    // Read the filelist into the buffer
    Read::by_ref(&mut origin).read_to_end(&mut file_bytes)?;

    // Return the read bytes
    Ok(file_bytes)
}

fn write_pyfile(filepath: &Path, data: Vec<u8>) -> Result<bool, BlackError> {
    // Setup a temporary, writable file to dump the supplied data into
    let mut temp = NamedTempFile::new()?;

    // Dump the supplied data to disk
    if temp.write_all(&data).is_err() {
        return Err(BlackError {
            what_happened: "Could not persist reformatted code to disk!".to_string(),
        });
    }

    // Replace the specified file with the written one
    return match temp.persist(filepath) {
        Ok(_) => Ok(true),
        Err(err) => Err(BlackError {
            what_happened: format!("{}", err),
        }),
    };
}

fn format_pyfile<T: AsRef<str>>(filepath: T, client: RequestBuilder) -> Result<bool, BlackError> {
    let filepath = PathBuf::from(filepath.as_ref())
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filepath.as_ref()));

    if !filepath.exists() {
        return Ok(false);
    }

    let client = client.body(read_pyfile(filepath.as_path())?);

    let resp = client.send()?;

    match resp.status() {
        StatusCode::OK => {
            return match write_pyfile(filepath.as_path(), resp.bytes().unwrap().to_vec()) {
                Ok(val) => {
                    if val {
                        println!("Successfully reformatted {:?}", filepath);
                        Ok(true)
                    } else {
                        println!("Could not reformat {:?}", filepath);
                        Ok(false)
                    }
                }
                Err(err) => Err(err),
            }
        }
        StatusCode::NO_CONTENT => {
            println!("{:?} already well formatted, good job.", filepath);
            return Ok(false);
        }
        StatusCode::BAD_REQUEST => Err(BlackError {
            what_happened: String::from_utf8(resp.bytes().unwrap().to_vec())?,
        }),
        StatusCode::INTERNAL_SERVER_ERROR => Err(BlackError {
            what_happened: format!("{:?} caused an internal error in `blackd`", filepath),
        }),
        _ => Err(BlackError {
            what_happened: format!(
                "`blackd` returned an unrecognized status code: {}",
                resp.status()
            ),
        }),
    }
}
