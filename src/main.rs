use clap::Parser;
use git2::Repository;
use log::{debug, info, warn};
use regex::Regex;
use semver::Version;
use std::error::Error;
use std::fs;
use std::path::{Component, PathBuf};

const DEFAULT_TEMPLATE: &'static str = "v{version}";
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "git-tag")]
    input: String,

    #[arg(long, default_value = "")]
    input_template: String,

    #[arg(long, default_value = "main")]
    input_branch: String,

    #[arg(long, default_value = DEFAULT_TEMPLATE)]
    output_template: String,

    #[arg(long, default_value_t = false)]
    major: bool,

    #[arg(long, default_value_t = false)]
    minor: bool,

    #[arg(long, default_value_t = false)]
    patch: bool,

    #[arg(short, long, default_value_t = true)]
    conventional_commits: bool,

    #[arg(long = "set", value_parser = parse_key_val::<String, String>)]
    vars: Vec<(String, String)>,

    #[arg(default_value = ".")]
    path: PathBuf,
}

fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

const SEMVER_RES: &'static str = r"((0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*))";
const VARS_RES: &'static str = r"\{.*?\}";

fn get_version_from_git(
    path: &PathBuf,
    template: &str,
    _branch: &str,
    vars: &Vec<(String, String)>,
) -> String {
    // TODO: branch based tag? Make sure that tags (commits) exists on the specified branch.
    let repo = match Repository::discover(path) {
        Ok(repo) => repo,
        Err(e) => panic!("Failed to open {}", e),
    };

    let mut version = Version::parse("0.0.0").unwrap();

    let vars_regex = Regex::new(VARS_RES).unwrap();

    let version_res_input = template.replace("{version}", SEMVER_RES);
    let version_res_input = render_template(&version_res_input, vars);
    debug!("Render template for version input: {}", version_res_input);
    let version_res_input = vars_regex.replace_all(&version_res_input, ".*?");
    debug!("Pattern used for version matching: {}", version_res_input);

    let version_regex = Regex::new(version_res_input.as_ref()).unwrap();

    let tag_pattern = vars_regex.replace_all(template, "*");
    debug!("Pattern used for tag matching: {}", tag_pattern);
    let tags = repo.tag_names(Some(&tag_pattern)).unwrap();

    debug!("Found {} possible tags candidates.", tags.len());
    for tag in tags.iter() {
        let tag = tag.unwrap();
        match version_regex.captures(tag) {
            Some(cap) => {
                let tag_version = Version::parse(&cap[1]).unwrap();
                debug!("Checking tag {}", tag_version);
                if tag_version > version {
                    version = tag_version;
                }
            }
            None => {
                warn!("Version not found in tag: {}", tag);
            }
        }
    }

    info!("Current version: {}", version);
    version.to_string()
}

fn render_template(template: &str, vars: &Vec<(String, String)>) -> String {
    let mut output = String::from(template);
    for var in vars.iter() {
        let name = "{".to_string() + &var.0 + "}";
        debug!("Replace {} with {}", &var.0, &var.1);
        output = output.replace(&name, &var.1.as_ref());
    }

    info!("Render template {} to {}", template, output);
    output
}

fn add_path_to_vars(path: &PathBuf, vars: &mut Vec<(String, String)>) {
    info!("Add path {} to vars list", path.to_str().unwrap());
    // TODO: stop adding component when in the root of git directory (to not expose the rest of the path)?
    let max = path.components().count() - 1;
    let mut crt = 0;
    for component in path.components() {
        if component == Component::RootDir {
            continue;
        }
        let part = component.as_os_str().to_str().unwrap();
        vars.push((format!("path[{}]", crt), String::from(part)));
        vars.push((format!("path[-{}]", max - crt), String::from(part)));
        crt += 1;
    }
}

fn increment_version(version: &mut Version, args: &Args) {
    if args.major {
        version.major += 1;
        version.minor = 0;
        version.patch = 0;
    }
    if args.minor {
        version.minor += 1;
        version.patch = 0;
    }
    if args.patch {
        version.patch += 1;
    }

    if args.conventional_commits {
        increment_conventional_version(version);
    }
}

fn increment_conventional_version(version: &mut Version) {
    info!("Increment version using conventional commits");
    // TODO: increment version based on conventional commits
    version.minor += 1;
}

fn normalize_inputs(args: &mut Args) {
    args.path = match fs::canonicalize(&args.path) {
        Ok(path) => path,
        Err(e) => {
            println!("Error: {}", e);
            std::process::exit(1)
        }
    };

    if args.input_template.is_empty() {
        info!(
            "Input template is empty. Filling it from output template {}",
            args.output_template.as_str()
        );
        args.input_template = args.output_template.clone();
    }

    if args.major || args.minor || args.patch {
        info!("Major/minor/patch provided. Disabling conventional commits check.");
        args.conventional_commits = false;
    }
}

fn main() {
    env_logger::init();

    let mut args = Args::parse();
    normalize_inputs(&mut args);
    add_path_to_vars(&args.path, &mut args.vars);

    let version = match args.input.as_ref() {
        "git-tag" => get_version_from_git(
            &args.path,
            &args.input_template,
            &args.input_branch,
            &args.vars,
        ),
        // TODO: toml, ini
        _ => {
            println!("Unsupported option for --input: {}", &args.input);
            std::process::exit(1)
        }
    };

    let mut version = Version::parse(version.as_ref()).unwrap();

    increment_version(&mut version, &args);

    info!("Next version: {}", version);
    args.vars
        .push((String::from("version"), version.to_string()));

    let version = render_template(&args.output_template, &args.vars);

    println!("{}", version);
}
