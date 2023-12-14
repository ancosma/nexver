#![forbid(unsafe_code)]

use clap::Parser;
use git2::{DiffDelta, DiffFormat, DiffHunk, DiffLine, DiffOptions, Oid, Repository};
use log::{debug, info, warn};
use regex::Regex;
use semver::Version;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::str;

extern crate git_conventional;

const DEFAULT_TEMPLATE: &'static str = "v{version}";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, default_value = "not-in-use")]
    _base_ref: String,

    #[arg(long, default_value = "HEAD")]
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

type Vars = HashMap<String, String>;

struct Config {
    head: String,

    input: String,

    output: String,

    vars: Vars,

    major_types: Vec<String>,

    minor_types: Vec<String>,

    patch_types: Vec<String>,

    path: PathBuf,
}

impl TryInto<Config> for Args {
    type Error = Box<dyn Error>;

    fn try_into(self) -> Result<Config, Self::Error> {
        Ok(Config {
            head: self.head_ref,
            input: if self.input_template.is_empty() {
                self.output_template.clone()
            } else {
                self.input_template
            },
            output: self.output_template,
            vars: self.vars.into_iter().map(|x| (x.0, x.1)).collect(),
            major_types: self.major_types,
            minor_types: self.minor_types,
            patch_types: self.patch_types,
            path: self.path.canonicalize()?,
        })
    }
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

fn get_version_from_git(repo: &Repository, template: &str, vars: &Vars) -> (Version, Oid, String) {
    let mut version = Version::new(0, 0, 0);

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

    let mut oid = Oid::zero();
    let mut found_tag = String::new();
    debug!("Found {} possible tags candidates.", tags.len());
    for tag in tags.iter() {
        let tag = tag.unwrap();
        match version_regex.captures(tag) {
            Some(cap) => {
                let tag_version = Version::parse(&cap[1]).unwrap();
                debug!("Checking tag {}", tag_version);
                if tag_version >= version {
                    version = tag_version;
                    found_tag = tag.to_string();
                }
            }
            None => {
                warn!("Version not found in tag: {}", tag);
            }
        }
    }
    if let Ok(r) = repo.revparse_single(&found_tag) {
        oid = r.id();
    }

    (version, oid, found_tag)
}

fn render_template(template: &str, vars: &Vars) -> String {
    let mut output = String::from(template);
    for var in vars.iter() {
        let name = "{".to_string() + &var.0 + "}";
        debug!("Replace {} with {}", &var.0, &var.1);
        output = output.replace(&name, &var.1.as_ref());
    }

    info!("Render template {} to {}", template, output);
    output
}

fn add_path_to_vars(path: &Path, vars: &mut Vars) -> Result<(), Box<dyn Error>> {
    info!("Add path {} to vars list", path.to_str().unwrap());
    for (i, component) in path.components().rev().enumerate() {
        let part = component.as_os_str().to_str().unwrap();
        vars.insert(format!("path[{}]", i + 1), String::from(part));
        vars.insert(format!("path[{}]", -1 - i as i32), String::from(part));
    }
    Ok(())
}

fn is_not_file_on_path(
    _delta: DiffDelta,
    _hunk: Option<DiffHunk>,
    line: DiffLine,
    path: &Path,
) -> bool {
    !line
        .content()
        .starts_with(path.to_str().unwrap().as_bytes())
}

fn increment_version(
    version: &mut Version,
    tag: &str,
    config: &Config,
    repo: &Repository,
) -> Result<(), Box<dyn Error>> {
    let mut major = false;
    let mut minor = false;
    let mut patch = false;

    let path = config.path.strip_prefix(repo.workdir().unwrap())?;
    let mut rw = repo.revwalk().expect("Failed to walk git commits.");
    let head = repo.revparse_single(&config.head).unwrap().id();
    if !tag.is_empty() {
        let tag_id = repo.revparse_single(&tag)?.id();
        info!("Checking commits between: {}..{}", tag_id, head);
        rw.push_range(format!("{}..{}", tag_id, head).as_str())?;
    } else {
        info!("Checking commits before: {}", head);
        rw.push(head)?;
    }
    let mut count = 0;
    for commit in rw {
        count += 1;
        let git_commit = repo.find_commit(commit?)?;
        if git_commit.parent_count() == 0 {
            break;
        }

        let tree = git_commit.tree();
        let prev_tree = repo
            .revparse_single(format!("{}^", git_commit.id()).as_str())?
            .peel(git2::ObjectType::Tree)?;
        let mut options = DiffOptions::new();
        let diff =
            repo.diff_tree_to_tree(prev_tree.as_tree(), tree.ok().as_ref(), Some(&mut options))?;
        if let Ok(_) = diff.print(DiffFormat::NameOnly, |d, h, l| {
            is_not_file_on_path(d, h, l, path)
        }) {
            info!(
                "Commit {} contain no file changed for path {:?}",
                git_commit.id(),
                path
            );
            continue;
        }
        let message = git_commit.message().ok_or("")?;
        match git_conventional::Commit::parse(&message) {
            Ok(info) => {
                let t: String;
                if info.scope().is_some() {
                    t = format!(
                        "{}({}){}",
                        info.type_(),
                        info.scope().unwrap(),
                        if info.breaking() { "!" } else { "" }
                    );
                } else {
                    t = format!("{}{}", info.type_(), if info.breaking() { "!" } else { "" });
                }
                if info.breaking() || config.major_types.contains(&t) {
                    major = true;
                    info!("{} commit contains a breaking change.", git_commit.id());
                    break;
                } else if config.minor_types.contains(&t) {
                    info!("{} commit contains a minor change.", git_commit.id());
                    minor = true;
                } else if config.patch_types.contains(&t) {
                    info!("{} commit contains a patch change.", git_commit.id());
                    patch = true;
                } else {
                    info!(
                        "{} commit of type {} skipped",
                        git_commit.id(),
                        info.type_().to_string(),
                    );
                }
            }
            Err(_e) => {
                warn!(
                    "Skipping commit {} due to commit message error.",
                    git_commit.id()
                );
            }
        }
    }
    info!("Checked {} commits.", count);
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
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut config: Config = Args::parse().try_into()?;
    info!("Path: {:?}", &config.path);
    let repo = Repository::discover(&config.path)?;

    let workdir = repo
        .workdir()
        .ok_or("Bare repos not supported.".to_string())?;
    let parent_path = workdir.join("../").canonicalize()?;
    add_path_to_vars(config.path.strip_prefix(parent_path)?, &mut config.vars)?;
    let (mut version, commit, tag) = get_version_from_git(&repo, &config.input, &config.vars);
    config
        .vars
        .insert("previous-version".to_string(), version.to_string());
    config
        .vars
        .insert("previous-tag".to_string(), tag.to_string());
    debug!("Version: {} Commit: {} Tag: {}", version, commit, tag);

    increment_version(&mut version, &tag, &config, &repo)?;

    info!("Next version: {}", version);
    config
        .vars
        .insert(String::from("version"), version.to_string());
    config.vars.insert(
        "tag".to_string(),
        config
            .input
            .replace("{version}", version.to_string().as_str()),
    );

    println!("{}", render_template(&config.output, &config.vars));
    Ok(())
}
