use clap::Parser;
use core::panic;
use git2::Repository;
use log::{debug, error, info, warn};
use regex::Regex;
use semver::Version;
use std::error::Error;
use std::path::{Component, PathBuf};
extern crate git_conventional;
const DEFAULT_TEMPLATE: &'static str = "v{version}";
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "main")]
    base_ref: String,

    #[arg(long, default_value = "main^")]
    head_ref: String,

    #[arg(long, default_value = "")]
    input_template: String,

    #[arg(long, default_value = DEFAULT_TEMPLATE)]
    output_template: String,

    #[arg(long = "set", value_parser = parse_key_val::<String, String>)]
    vars: Vec<(String, String)>,

    #[arg(long, value_parser, num_args=1.., value_delimiter=',')]
    major_types: Vec<String>,

    #[arg(long, default_value = "feat", value_parser, num_args=1.., value_delimiter=',')]
    minor_types: Vec<String>,

    #[arg(long, default_value = "fix", value_parser, num_args=1.., value_delimiter=',')]
    patch_types: Vec<String>,

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

fn get_version_from_git(repo: &Repository, template: &str, vars: &Vec<(String, String)>) -> String {
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

fn increment_version(version: &mut Version, args: &Args, repo: &Repository) {
    let mut major = false;
    let mut minor = false;
    let mut patch = false;

    let mut rw = repo.revwalk().expect("Failed to walk git commits.");
    rw.push_range(format!("{}..{}", &args.head_ref, &args.base_ref).as_str())
        .expect("Checking commits failed.");
    for commit in rw {
        let c: Result<git2::Commit, git2::Error> = match commit {
            Ok(oid) => repo.find_commit(oid),
            Err(err) => {
                panic!("{}", err);
            }
        };
        let git_commit = c.unwrap();
        let message = git_commit.message().unwrap();
        // TODO: check if commit changes are at current specified path
        match git_conventional::Commit::parse(&message) {
            Ok(info) => {
                if info.breaking() || args.major_types.contains(&info.type_().to_string()) {
                    major = true;
                    info!("{} commit contains a breaking change.", git_commit.id());
                    break;
                }
                if args.minor_types.contains(&info.type_().to_string()) {
                    minor = true;
                }
                if args.patch_types.contains(&info.type_().to_string()) {
                    patch = true;
                }
            }
            Err(_e) => {}
        }
    }
    let (major, minor, patch) = (major, minor, patch);
    match (major, minor, patch) {
        (true, _, _) => {
            version.major += 1;
            version.minor = 0;
            version.patch = 0;
        }
        (_, true, _) => {
            version.minor += 1;
            version.patch = 0;
        }
        (_, _, true) => {
            version.patch += 1;
        }
        _ => {}
    }
}

fn normalize_inputs(args: &mut Args) {
    args.path = match args.path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            error!("Error: {}", e);
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
}

fn main() {
    env_logger::init();

    let mut args = Args::parse();
    normalize_inputs(&mut args);
    add_path_to_vars(&args.path, &mut args.vars);
    let repo = match Repository::discover(&args.path) {
        Ok(repo) => repo,
        Err(e) => panic!("Failed to open {}", e),
    };

    let version = get_version_from_git(&repo, &args.input_template, &args.vars);

    let mut version = Version::parse(version.as_ref()).unwrap();

    increment_version(&mut version, &args, &repo);

    info!("Next version: {}", version);
    args.vars
        .push((String::from("version"), version.to_string()));

    let version = render_template(&args.output_template, &args.vars);

    println!("{}", version);
}
